use std::io::{Read, Write};

/// LZSS.C -- A Data Compression Program
/// (tab = 4 spaces)
/// 
/// 4/6/1989 Haruhiko Okumura
/// Use, distribute, and modify this program freely.
/// Please send me your improved versions.
///     PC-VAN      SCIENCE
///     NIFTY-Serve PAF01022
///     CompuServe  74050,1022
pub struct Lzss {
    /// size of ring buffer
    text_buf: [u8; Self::N + Self::F - 1],
    /// of longest match. These are set by the insert_node() procedure.
    match_position: usize,
    match_length: usize,
    /// left & right children & parents -- These constitute binary search trees.
    lson: [usize; Self::N + 1],
    rson: [usize; Self::N + 257],
    dad: [usize; Self::N + 1],
}

impl Lzss {
    const N: usize = 2048; // size of ring buffer
    const F: usize = 24;   // upper limit for match_length
    const THRESHOLD: usize = 1; // encode string into position and length if match_length is greater than this
    const NIL: usize = Self::N; // index for root of binary search trees

    pub fn new() -> Self {
        Self {
            text_buf: [0; Self::N + Self::F - 1],
            match_position: 0,
            match_length: 0,
            lson: [0; Self::N + 1],
            rson: [0; Self::N + 257],  
            dad: [0; Self::N + 1],
        }
    }

    /// Initialize trees
    fn init_tree(&mut self) {
        // For i = 0 to N - 1, rson[i] and lson[i] will be the right and
        // left children of node i.  These nodes need not be initialized.
        // Also, dad[i] is the parent of node i.  These are initialized to
        // NIL (= N), which stands for 'not used.'
        // For i = 0 to 255, rson[N + i + 1] is the root of the tree
        // for strings that begin with character i.  These are initialized
        // to NIL.  Note there are 256 trees.

        for i in (Self::N + 1)..=(Self::N + 256) {
            self.rson[i] = Self::NIL;
        }
        for i in 0..Self::N {
            self.dad[i] = Self::NIL;
        }
    }

    /// Inserts string of length F, text_buf[r..r+F-1], into one of the
    /// trees (text_buf[r]'th tree) and returns the longest-match position
    /// and length via the global variables match_position and match_length.
    /// If match_length = F, then removes the old node in favor of the new
    /// one, because the old one will be deleted sooner.
    /// Note r plays double role, as tree node and position in buffer.
    fn insert_node(&mut self, r: usize) {
        let mut cmp = 1i32;
        let key = r;
        let mut p = Self::N + 1 + self.text_buf[key] as usize;
        
        self.rson[r] = Self::NIL;
        self.lson[r] = Self::NIL;
        self.match_length = 0;
        
        loop {
            if cmp >= 0 {
                if self.rson[p] != Self::NIL {
                    p = self.rson[p];
                } else {
                    self.rson[p] = r;
                    self.dad[r] = p;
                    return;
                }
            } else {
                if self.lson[p] != Self::NIL {
                    p = self.lson[p];
                } else {
                    self.lson[p] = r;
                    self.dad[r] = p;
                    return;
                }
            }
            
            let mut i = 1;
            while i < Self::F {
                cmp = self.text_buf[key + i] as i32 - self.text_buf[p + i] as i32;
                if cmp != 0 {
                    break;
                }
                i += 1;
            }
            
            if i > self.match_length {
                self.match_position = p;
                self.match_length = i;
                if self.match_length >= Self::F {
                    break;
                }
            }
        }
        
        self.dad[r] = self.dad[p];
        self.lson[r] = self.lson[p];
        self.rson[r] = self.rson[p];
        self.dad[self.lson[p]] = r;
        self.dad[self.rson[p]] = r;
        
        if self.rson[self.dad[p]] == p {
            self.rson[self.dad[p]] = r;
        } else {
            self.lson[self.dad[p]] = r;
        }
        
        self.dad[p] = Self::NIL; // remove p
    }

    /// Deletes node p from tree
    fn delete_node(&mut self, p: usize) {
        if self.dad[p] == Self::NIL {
            return; // not in tree
        }
        
        let q = if self.rson[p] == Self::NIL {
            self.lson[p]
        } else if self.lson[p] == Self::NIL {
            self.rson[p]
        } else {
            let mut q = self.lson[p];
            if self.rson[q] != Self::NIL {
                while self.rson[q] != Self::NIL {
                    q = self.rson[q];
                }
                self.rson[self.dad[q]] = self.lson[q];
                self.dad[self.lson[q]] = self.dad[q];
                self.lson[q] = self.lson[p];
                self.dad[self.lson[p]] = q;
            }
            self.rson[q] = self.rson[p];
            self.dad[self.rson[p]] = q;
            q
        };
        
        self.dad[q] = self.dad[p];
        if self.rson[self.dad[p]] == p {
            self.rson[self.dad[p]] = q;
        } else {
            self.lson[self.dad[p]] = q;
        }
        self.dad[p] = Self::NIL;
    }

    fn encode<R: Read, W: Write>(&mut self, mut input: R, mut output: W) -> std::io::Result<()> {
        let mut code_buf = [0u8; 17];
        let mut mask: u8;

        self.init_tree(); // initialize trees
        
        code_buf[0] = 0; // code_buf[1..16] saves eight units of code, and
                        // code_buf[0] works as eight flags, "1" representing that the unit
                        // is an unencoded letter (1 byte), "0" a position-and-length pair
                        // (2 bytes). Thus, eight units require at most 16 bytes of code.
        
        let mut code_buf_ptr = 1;
        mask = 1;
        let s = 0;
        let mut r = Self::N - Self::F;
        
        // Clear the buffer with any character that will appear often.
        for i in s..r {
            self.text_buf[i] = 0;
        }
        
        // Read F bytes into the last F bytes of the buffer
        let mut len = 0;
        let mut buffer = [0u8; 1];
        while len < Self::F {
            match input.read(&mut buffer)? {
                0 => break, // EOF
                _ => {
                    self.text_buf[r + len] = buffer[0];
                    len += 1;
                }
            }
        }
        
        if len == 0 {
            return Ok(()); // text of size zero
        }
        
        // Insert the F strings, each of which begins with one or more 'space' characters.
        // Note the order in which these strings are inserted. This way,
        // degenerate trees will be less likely to occur.
        for i in 1..=Self::F {
            self.insert_node(r.wrapping_sub(i));
        }
        
        // Finally, insert the whole string just read. The
        // global variables match_length and match_position are set.
        self.insert_node(r);
        
        let mut s = s;
        
        loop {
            if self.match_length > len {
                self.match_length = len; // match_length may be spuriously long near the end of text.
            }
            
            if self.match_length <= Self::THRESHOLD {
                self.match_length = 1; // Not long enough match. Send one byte.
                code_buf[0] |= mask; // 'send one byte' flag
                code_buf[code_buf_ptr] = self.text_buf[r]; // Send uncoded.
                code_buf_ptr += 1;
            } else {
                code_buf[code_buf_ptr] = self.match_position as u8;
                code_buf_ptr += 1;
                code_buf[code_buf_ptr] = (((self.match_position >> 3) & 0xe0) | 
                                        (self.match_length - (Self::THRESHOLD + 1))) as u8;
                code_buf_ptr += 1;
            }
            
            mask <<= 1;
            if mask == 0 { // Shift mask left one bit.
                // Send at most 8 units of code together
                for i in 0..code_buf_ptr {
                    output.write_all(&[code_buf[i]])?;
                }
                code_buf[0] = 0;
                code_buf_ptr = 1;
                mask = 1;
            }
            
            let last_match_length = self.match_length;
            let mut i = 0;
            
            while i < last_match_length {
                match input.read(&mut buffer)? {
                    0 => break, // EOF
                    _ => {
                        self.delete_node(s); // Delete old strings and
                        self.text_buf[s] = buffer[0]; // read new bytes
                        
                        if s < Self::F - 1 {
                            self.text_buf[s + Self::N] = buffer[0]; // If the position is
                                                                   // near the end of buffer, extend the buffer to make
                                                                   // string comparison easier.
                        }
                        
                        s = (s + 1) & (Self::N - 1);
                        r = (r + 1) & (Self::N - 1);
                        // Since this is a ring buffer, increment the position modulo N.
                        
                        self.insert_node(r); // Register the string in text_buf[r..r+F-1]
                        i += 1;
                    }
                }
            }
            
            while i < last_match_length { // After the end of text,
                self.delete_node(s); // no need to read, but
                s = (s + 1) & (Self::N - 1);
                r = (r + 1) & (Self::N - 1);
                len -= 1;
                if len != 0 {
                    self.insert_node(r); // buffer may not be empty.
                }
                i += 1;
            }
            
            if len == 0 {
                break; // until length of string to be processed is zero
            }
        }
        
        if code_buf_ptr > 1 { // Send remaining code.
            for i in 0..code_buf_ptr {
                output.write_all(&[code_buf[i]])?;
            }
        }
        
        Ok(())
    }

    /// Just the reverse of encode()
    fn decode<R: Read, W: Write>(&mut self, mut input: R, mut output: W) -> std::io::Result<()> {
        for i in 0..(Self::N - Self::F) {
            self.text_buf[i] = 0;
        }
        
        let mut r = Self::N - Self::F;
        let mut flags = 0u32;
        let mut buffer = [0u8; 1];
        
        loop {
            flags >>= 1;
            if (flags & 256) == 0 {
                match input.read(&mut buffer)? {
                    0 => break, // EOF
                    _ => {
                        flags = (buffer[0] as u32) | 0xff00; // uses higher byte cleverly to count eight
                    }
                }
            }
            
            if (flags & 1) != 0 {
                match input.read(&mut buffer)? {
                    0 => break, // EOF
                    _ => {
                        output.write_all(&[buffer[0]])?;
                        self.text_buf[r] = buffer[0];
                        r += 1;
                        r &= Self::N - 1;
                    }
                }
            } else {
                let i = match input.read(&mut buffer)? {
                    0 => break, // EOF
                    _ => buffer[0] as usize,
                };
                
                let j = match input.read(&mut buffer)? {
                    0 => break, // EOF
                    _ => buffer[0] as usize,
                };
                
                let pos = i | ((j & 0xe0) << 3);
                let length = (j & 0x1f) + Self::THRESHOLD;
                
                for k in 0..=length {
                    let c = self.text_buf[(pos + k) & (Self::N - 1)];
                    output.write_all(&[c])?;
                    self.text_buf[r] = c;
                    r += 1;
                    r &= Self::N - 1;
                }
            }
        }
        
        Ok(())
    }

    pub fn compress(&mut self, buffer: &[u8]) -> std::io::Result<Vec<u8>> {
        let input = std::io::Cursor::new(buffer);
        let mut output = Vec::new();
        
        self.encode(input, &mut output)?;
        
        Ok(output)
    }

    pub fn decompress(&mut self, buffer: &[u8]) -> std::io::Result<Vec<u8>> {
        let input = std::io::Cursor::new(buffer);
        let mut output = Vec::new();
        
        self.decode(input, &mut output)?;
        
        Ok(output)
    }
}

impl Default for Lzss {
    fn default() -> Self {
        Self::new()
    }
}
