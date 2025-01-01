from dataclasses import dataclass
from typing import Optional
import array

@dataclass
class LzssSettings:
    frame_size: int = 0x1000
    frame_fill: int = 0
    frame_init_pos: int = 0xFEE
    max_match_length: int = 0x12
    min_match_length: int = 2

class LZSSEncoder:
    def __init__(self, settings: Optional[LzssSettings] = None):
        settings = settings if settings else LzssSettings()
        self.settings = settings
        self.N = settings.frame_size  # Size of ring buffer
        self.F = settings.max_match_length  # Upper limit for match_length
        self.THRESHOLD = settings.min_match_length  # Encode string into position and length if match_length is greater than this
        self.NIL = self.N  # Index for root of binary search trees
        self.PADDING = settings.frame_fill  # Padding character

        # Ring buffer
        self.text_buf = array.array('B', [self.PADDING] * (self.N + self.F - 1))

        # Tree structures
        self.lchild = array.array('i', [0] * (self.N + 1))
        self.rchild = array.array('i', [0] * (self.N + 257))
        self.parent = array.array('i', [0] * (self.N + 1))

        # Match properties
        self.match_position = 0
        self.match_length = 0

        self._init_state()

    def _init_state(self):
        """Initialize encoding state and trees"""
        for i in range(self.N + 1, self.N + 257):
            self.rchild[i] = self.NIL
        for i in range(self.N):
            self.parent[i] = self.NIL

    def _insert_node(self, r: int):
        """Insert string of length F into one of the trees"""
        cmp_val = 1
        key = self.text_buf[r:r + self.F]
        p = self.N + 1 + key[0]
        self.rchild[r] = self.lchild[r] = self.NIL
        self.match_length = 0

        while True:
            if cmp_val >= 0:
                if self.rchild[p] != self.NIL:
                    p = self.rchild[p]
                else:
                    self.rchild[p] = r
                    self.parent[r] = p
                    return
            else:
                if self.lchild[p] != self.NIL:
                    p = self.lchild[p]
                else:
                    self.lchild[p] = r
                    self.parent[r] = p
                    return

            i = 1
            while i < self.F:
                cmp_val = key[i] - self.text_buf[p + i]
                if cmp_val != 0:
                    break
                i += 1

            if i > self.match_length:
                self.match_position = p
                self.match_length = i
                if i >= self.F:
                    break

        self.parent[r] = self.parent[p]
        self.lchild[r] = self.lchild[p]
        self.rchild[r] = self.rchild[p]
        self.parent[self.lchild[p]] = r
        self.parent[self.rchild[p]] = r

        if self.rchild[self.parent[p]] == p:
            self.rchild[self.parent[p]] = r
        else:
            self.lchild[self.parent[p]] = r
        self.parent[p] = self.NIL

    def _delete_node(self, p: int):
        """Delete node p from tree"""
        if self.parent[p] == self.NIL:
            return

        if self.rchild[p] == self.NIL:
            q = self.lchild[p]
        elif self.lchild[p] == self.NIL:
            q = self.rchild[p]
        else:
            q = self.lchild[p]
            if self.rchild[q] != self.NIL:
                while self.rchild[q] != self.NIL:
                    q = self.rchild[q]
                self.rchild[self.parent[q]] = self.lchild[q]
                self.parent[self.lchild[q]] = self.parent[q]
                self.lchild[q] = self.lchild[p]
                self.parent[self.lchild[p]] = q
            self.rchild[q] = self.rchild[p]
            self.parent[self.rchild[p]] = q

        self.parent[q] = self.parent[p]
        if self.rchild[self.parent[p]] == p:
            self.rchild[self.parent[p]] = q
        else:
            self.lchild[self.parent[p]] = q
        self.parent[p] = self.NIL

def compress(data: bytes, settings: Optional[LzssSettings] = None) -> Optional[bytes]:
    """
    Compress the input data using LZSS algorithm.

    Args:
        data: Input bytes to compress
        settings: LzssSettings instance with compression parameters

    Returns:
        Compressed data or None if input data is empty
    """
    settings = settings if settings else LzssSettings()
    
    if not data:
        return None

    encoder = LZSSEncoder(settings)
    code_buf = bytearray(17)
    compressed = bytearray()

    code_buf[0] = 0
    code_buf_ptr = 1
    mask = 1

    s = 0
    r = encoder.N - encoder.F

    # Read initial F bytes
    data_pos = 0
    length = 0
    while length < encoder.F and data_pos < len(data):
        encoder.text_buf[r + length] = data[data_pos]
        data_pos += 1
        length += 1

    if length == 0:
        return None

    # Insert initial strings
    for i in range(1, encoder.F + 1):
        encoder._insert_node(r - i)

    encoder._insert_node(r)

    while length > 0:
        if encoder.match_length > length:
            encoder.match_length = length

        if encoder.match_length <= encoder.THRESHOLD:
            encoder.match_length = 1
            code_buf[0] |= mask
            code_buf[code_buf_ptr] = encoder.text_buf[r]
            code_buf_ptr += 1
        else:
            code_buf[code_buf_ptr] = encoder.match_position & 0xFF
            code_buf_ptr += 1
            code_buf[code_buf_ptr] = (
                ((encoder.match_position >> 4) & 0xF0) |
                (encoder.match_length - (encoder.THRESHOLD + 1))
            )
            code_buf_ptr += 1

        mask <<= 1

        if mask == 0x100:
            compressed.extend(code_buf[:code_buf_ptr])
            code_buf[0] = 0
            code_buf_ptr = 1
            mask = 1

        last_match_length = encoder.match_length
        i = 0

        while i < last_match_length and data_pos < len(data):
            encoder._delete_node(s)
            c = data[data_pos]
            data_pos += 1
            encoder.text_buf[s] = c

            if s < encoder.F - 1:
                encoder.text_buf[s + encoder.N] = c

            s = (s + 1) & (encoder.N - 1)
            r = (r + 1) & (encoder.N - 1)

            encoder._insert_node(r)
            i += 1

        while i < last_match_length:
            encoder._delete_node(s)
            s = (s + 1) & (encoder.N - 1)
            r = (r + 1) & (encoder.N - 1)
            length -= 1
            if length:
                encoder._insert_node(r)
            i += 1

    if code_buf_ptr > 1:
        compressed.extend(code_buf[:code_buf_ptr])

    return bytes(compressed)

def decompress(compressed_data: bytes, settings: Optional[LzssSettings] = None) -> Optional[bytes]:
    """
    Decompress LZSS compressed data.

    Args:
        compressed_data: LZSS compressed bytes
        settings: LzssSettings instance with decompression parameters

    Returns:
        Decompressed data or None if input data is empty
    """
    settings = settings if settings else LzssSettings()
    
    if not compressed_data:
        return None

    text_buf = array.array('B', [settings.frame_fill] * (settings.frame_size + settings.max_match_length - 1))
    decompressed = bytearray()

    N = settings.frame_size
    F = settings.max_match_length
    THRESHOLD = settings.min_match_length

    r = N - F
    flags = 0
    data_pos = 0

    while data_pos < len(compressed_data):
        flags >>= 1
        if (flags & 0x100) == 0:
            if data_pos < len(compressed_data):
                flags = compressed_data[data_pos] | 0xFF00
                data_pos += 1
            else:
                break

        if flags & 1:
            if data_pos < len(compressed_data):
                c = compressed_data[data_pos]
                data_pos += 1
                decompressed.append(c)
                text_buf[r] = c
                r = (r + 1) & (N - 1)
            else:
                break
        else:
            if data_pos + 1 < len(compressed_data):
                i = compressed_data[data_pos]
                j = compressed_data[data_pos + 1]
                data_pos += 2

                i |= ((j & 0xF0) << 4)
                j = (j & 0x0F) + THRESHOLD

                for k in range(j + 1):
                    c = text_buf[(i + k) & (N - 1)]
                    decompressed.append(c)
                    text_buf[r] = c
                    r = (r + 1) & (N - 1)
            else:
                break

    return bytes(decompressed)
