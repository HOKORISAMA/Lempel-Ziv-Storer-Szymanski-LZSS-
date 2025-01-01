package lzss

// LzssSettings and DefaultSettings implementations remain unchanged
type LzssSettings struct {
	FrameSize      int
	FrameFill      byte
	FrameInitPos   int
	MaxMatchLength int
	MinMatchLength int
}

func DefaultSettings() *LzssSettings {
	return &LzssSettings{
		FrameSize:      0x1000,
		FrameFill:      0,
		FrameInitPos:   0xFEE,
		MaxMatchLength: 0x12,
		MinMatchLength: 2,
	}
}

// LZSSEncoder implements LZSS compression
type LZSSEncoder struct {
	settings      *LzssSettings
	N             int    // Size of ring buffer
	F             int    // Upper limit for match_length
	THRESHOLD     int    // Minimum length threshold for encoding
	NIL           int    // Index for root of binary search trees
	PADDING       byte   // Padding character
	textBuf       []byte // Ring buffer
	lchild        []int  // Left children
	rchild        []int  // Right children
	parent        []int  // Parent nodes
	matchPosition int    // Current match position
	matchLength   int    // Current match length
}

// NewEncoder creates a new LZSS encoder with given settings
func NewEncoder(settings *LzssSettings) *LZSSEncoder {
	if settings == nil {
		settings = DefaultSettings()
	}

	e := &LZSSEncoder{
		settings:  settings,
		N:         settings.FrameSize,
		F:         settings.MaxMatchLength,
		THRESHOLD: settings.MinMatchLength,
		NIL:       settings.FrameSize,
		PADDING:   settings.FrameFill,
	}

	// Initialize buffers
	e.textBuf = make([]byte, e.N+e.F-1)
	for i := range e.textBuf {
		e.textBuf[i] = e.PADDING
	}

	e.lchild = make([]int, e.N+1)
	e.rchild = make([]int, e.N+257)
	e.parent = make([]int, e.N+1)

	e.initState()
	return e
}

func (e *LZSSEncoder) initState() {
	// Initialize encoding state and trees
	for i := e.N + 1; i < e.N+257; i++ {
		e.rchild[i] = e.NIL
	}
	for i := 0; i < e.N; i++ {
		e.parent[i] = e.NIL
	}
}

func (e *LZSSEncoder) insertNode(r int) {
	var i int
	var p int
	var cmp int

	key := e.textBuf[r : r+e.F]
	p = e.N + 1 + int(key[0])
	e.rchild[r] = e.NIL
	e.lchild[r] = e.NIL
	e.matchLength = 0

	for {
		if cmp >= 0 {
			if e.rchild[p] != e.NIL {
				p = e.rchild[p]
			} else {
				e.rchild[p] = r
				e.parent[r] = p
				return
			}
		} else {
			if e.lchild[p] != e.NIL {
				p = e.lchild[p]
			} else {
				e.lchild[p] = r
				e.parent[r] = p
				return
			}
		}

		i = 1
		for i < e.F {
			cmp = int(key[i]) - int(e.textBuf[p+i])
			if cmp != 0 {
				break
			}
			i++
		}

		if i > e.matchLength {
			e.matchPosition = p
			e.matchLength = i
			if i >= e.F {
				break
			}
		}
	}

	e.parent[r] = e.parent[p]
	e.lchild[r] = e.lchild[p]
	e.rchild[r] = e.rchild[p]
	e.parent[e.lchild[p]] = r
	e.parent[e.rchild[p]] = r

	if e.rchild[e.parent[p]] == p {
		e.rchild[e.parent[p]] = r
	} else {
		e.lchild[e.parent[p]] = r
	}
	e.parent[p] = e.NIL
}

func (e *LZSSEncoder) deleteNode(p int) {
	var q int

	if e.parent[p] == e.NIL {
		return
	}

	if e.rchild[p] == e.NIL {
		q = e.lchild[p]
	} else if e.lchild[p] == e.NIL {
		q = e.rchild[p]
	} else {
		q = e.lchild[p]
		if e.rchild[q] != e.NIL {
			for e.rchild[q] != e.NIL {
				q = e.rchild[q]
			}
			e.rchild[e.parent[q]] = e.lchild[q]
			e.parent[e.lchild[q]] = e.parent[q]
			e.lchild[q] = e.lchild[p]
			e.parent[e.lchild[p]] = q
		}
		e.rchild[q] = e.rchild[p]
		e.parent[e.rchild[p]] = q
	}

	e.parent[q] = e.parent[p]
	if e.rchild[e.parent[p]] == p {
		e.rchild[e.parent[p]] = q
	} else {
		e.lchild[e.parent[p]] = q
	}
	e.parent[p] = e.NIL
}

// Compress compresses input data using LZSS algorithm
func Compress(data []byte, settings *LzssSettings) []byte {
	if len(data) == 0 {
		return nil
	}

	if settings == nil {
		settings = DefaultSettings()
	}

	encoder := NewEncoder(settings)
	codeBuf := make([]byte, 17)
	compressed := make([]byte, 0)

	codeBuf[0] = 0
	codeBufPtr := 1
	mask := uint16(1)

	s := 0
	r := encoder.N - encoder.F

	// Read initial F bytes
	dataPos := 0
	length := 0
	for length < encoder.F && dataPos < len(data) {
		encoder.textBuf[r+length] = data[dataPos]
		dataPos++
		length++
	}

	if length == 0 {
		return nil
	}

	// Insert initial strings
	for i := 1; i <= encoder.F; i++ {
		encoder.insertNode(r - i)
	}
	encoder.insertNode(r)

	for length > 0 {
		if encoder.matchLength > length {
			encoder.matchLength = length
		}

		if encoder.matchLength <= encoder.THRESHOLD {
			encoder.matchLength = 1
			codeBuf[0] |= byte(mask)
			codeBuf[codeBufPtr] = encoder.textBuf[r]
			codeBufPtr++
		} else {
			codeBuf[codeBufPtr] = byte(encoder.matchPosition & 0xFF)
			codeBufPtr++
			codeBuf[codeBufPtr] = byte(((encoder.matchPosition >> 4) & 0xF0) |
				(encoder.matchLength - (encoder.THRESHOLD + 1)))
			codeBufPtr++
		}

		mask <<= 1

		if mask == 0x100 {
			compressed = append(compressed, codeBuf[:codeBufPtr]...)
			codeBuf[0] = 0
			codeBufPtr = 1
			mask = 1
		}

		lastMatchLength := encoder.matchLength
		i := 0

		for i < lastMatchLength && dataPos < len(data) {
			encoder.deleteNode(s)
			c := data[dataPos]
			dataPos++
			encoder.textBuf[s] = c

			if s < encoder.F-1 {
				encoder.textBuf[s+encoder.N] = c
			}

			s = (s + 1) & (encoder.N - 1)
			r = (r + 1) & (encoder.N - 1)

			encoder.insertNode(r)
			i++
		}

		for i < lastMatchLength {
			encoder.deleteNode(s)
			s = (s + 1) & (encoder.N - 1)
			r = (r + 1) & (encoder.N - 1)
			length--
			if length > 0 {
				encoder.insertNode(r)
			}
			i++
		}
	}

	if codeBufPtr > 1 {
		compressed = append(compressed, codeBuf[:codeBufPtr]...)
	}

	return compressed
}

// Decompress decompresses LZSS compressed data
func Decompress(compressedData []byte, settings *LzssSettings) []byte {
	if len(compressedData) == 0 {
		return nil
	}

	if settings == nil {
		settings = DefaultSettings()
	}

	textBuf := make([]byte, settings.FrameSize+settings.MaxMatchLength-1)
	for i := range textBuf {
		textBuf[i] = settings.FrameFill
	}

	decompressed := make([]byte, 0)

	N := settings.FrameSize
	F := settings.MaxMatchLength
	THRESHOLD := settings.MinMatchLength

	r := N - F
	flags := uint16(0)
	dataPos := 0

	for dataPos < len(compressedData) {
		flags >>= 1
		if (flags & 0x100) == 0 {
			if dataPos < len(compressedData) {
				flags = uint16(compressedData[dataPos]) | 0xFF00
				dataPos++
			} else {
				break
			}
		}

		if flags&1 != 0 {
			if dataPos < len(compressedData) {
				c := compressedData[dataPos]
				dataPos++
				decompressed = append(decompressed, c)
				textBuf[r] = c
				r = (r + 1) & (N - 1)
			} else {
				break
			}
		} else {
			if dataPos+1 < len(compressedData) {
				i := int(compressedData[dataPos])
				j := int(compressedData[dataPos+1])
				dataPos += 2

				i |= ((j & 0xF0) << 4)
				j = (j & 0x0F) + THRESHOLD

				for k := 0; k <= j; k++ {
					c := textBuf[(i+k)&(N-1)]
					decompressed = append(decompressed, c)
					textBuf[r] = c
					r = (r + 1) & (N - 1)
				}
			} else {
				break
			}
		}
	}

	return decompressed
}
