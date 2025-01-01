class LzssSettings {
  constructor() {
    this.frameSize = 0x1000;
    this.frameFill = 0;
    this.frameInitPos = 0xFEE;
    this.maxMatchLength = 0x12;
    this.minMatchLength = 2;
  }
}

class LzssCompression {
  constructor(input, isCompressing = false, settings = new LzssSettings()) {
    this.input = Buffer.from(input);
    this.inputPos = 0;
    this.isCompressing = isCompressing;
    this.settings = settings;
    this.output = Buffer.alloc(input.length * 2); // Allocate enough space for worst case
    this.outputPos = 0;
    this.buffer = new Uint8Array(settings.frameSize + settings.maxMatchLength - 1);
    
    if (settings.frameFill !== 0) {
      this.buffer.fill(settings.frameFill, 0, settings.frameSize);
    }
    
    if (isCompressing) {
      this.initCompress();
    }
  }

  readByte() {
    return this.inputPos < this.input.length ? this.input[this.inputPos++] : -1;
  }

  writeByte(byte) {
    this.output[this.outputPos++] = byte;
  }

  initCompress() {
    this.lc = new Int32Array(this.settings.frameSize + 1);
    this.rc = new Int32Array(this.settings.frameSize + 257);
    this.parents = new Int32Array(this.settings.frameSize + 1);
    this.matchLength = 0;
    this.matchPosition = 0;
  }

  initTree() {
    for (let i = this.settings.frameSize + 1; i <= this.settings.frameSize + 256; i++) {
      this.rc[i] = this.settings.frameSize;
    }
    for (let i = 0; i < this.settings.frameSize; i++) {
      this.parents[i] = this.settings.frameSize;
    }
  }

  insertNode(r) {
    let i = 0;
    let p = this.settings.frameSize + 1 + this.buffer[r];
    let cmp = 1;
    this.rc[r] = this.lc[r] = this.settings.frameSize;
    this.matchLength = 0;

    while (true) {
      if (cmp >= 0) {
        if (this.rc[p] !== this.settings.frameSize) {
          p = this.rc[p];
        } else {
          this.rc[p] = r;
          this.parents[r] = p;
          return;
        }
      } else {
        if (this.lc[p] !== this.settings.frameSize) {
          p = this.lc[p];
        } else {
          this.lc[p] = r;
          this.parents[r] = p;
          return;
        }
      }

      for (i = 1; i < this.settings.maxMatchLength; i++) {
        if ((cmp = this.buffer[r + i] - this.buffer[p + i]) !== 0) {
          break;
        }
      }

      if (i > this.matchLength) {
        this.matchPosition = p;
        if ((this.matchLength = i) >= this.settings.maxMatchLength) {
          break;
        }
      }
    }
  }

  deleteNode(p) {
    let q;
    if (this.parents[p] === this.settings.frameSize) {
      return;
    }
    if (this.rc[p] === this.settings.frameSize) {
      q = this.lc[p];
    } else if (this.lc[p] === this.settings.frameSize) {
      q = this.rc[p];
    } else {
      q = this.lc[p];
      if (this.rc[q] !== this.settings.frameSize) {
        do {
          q = this.rc[q];
        } while (this.rc[q] !== this.settings.frameSize);
        this.rc[this.parents[q]] = this.lc[q];
        this.parents[this.lc[q]] = this.parents[q];
        this.lc[q] = this.lc[p];
        this.parents[this.lc[p]] = q;
      }
      this.rc[q] = this.rc[p];
      this.parents[this.rc[p]] = q;
    }
    this.parents[q] = this.parents[p];
    if (this.rc[this.parents[p]] === p) {
      this.rc[this.parents[p]] = q;
    } else {
      this.lc[this.parents[p]] = q;
    }
    this.parents[p] = this.settings.frameSize;
  }

  decompress() {
    if (this.isCompressing) {
      throw new Error("Not in decompression mode");
    }

    let flag = 0;
    let byteRead, distance, length;

    while (true) {
      if (((flag >>= 1) & 256) === 0) {
        if ((byteRead = this.readByte()) === -1) break;
        flag = byteRead | 0xff00;
      }

      if ((flag & 1) !== 0) {
        if ((byteRead = this.readByte()) === -1) break;
        this.writeByte(byteRead);
        this.buffer[this.settings.frameInitPos] = byteRead;
        this.settings.frameInitPos = (this.settings.frameInitPos + 1) & (this.settings.frameSize - 1);
      } else {
        if ((distance = this.readByte()) === -1) break;
        if ((length = this.readByte()) === -1) break;

        distance |= (length & 0xf0) << 4;
        length = (length & 0x0f) + this.settings.minMatchLength;

        for (let k = 0; k <= length; k++) {
          byteRead = this.buffer[(distance + k) & (this.settings.frameSize - 1)];
          this.writeByte(byteRead);
          this.buffer[this.settings.frameInitPos] = byteRead;
          this.settings.frameInitPos = (this.settings.frameInitPos + 1) & (this.settings.frameSize - 1);
        }
      }
    }
    return Buffer.from(this.output.slice(0, this.outputPos));
  }

  compress() {
    if (!this.isCompressing) {
      throw new Error("Not in compression mode");
    }

    let r = this.settings.frameInitPos;
    let s = 0;
    let len = 0;
    let i, c, lastMatchLength;
    let codeBuf = new Uint8Array(17);
    let codeBufPtr = 1;
    let mask = 1;

    this.initTree();
    codeBuf[0] = 0;

    for (len = 0; len < this.settings.maxMatchLength && (c = this.readByte()) !== -1; len++) {
      this.buffer[r + len] = c;
    }
    if (len === 0) {
      return Buffer.alloc(0);
    }
    for (i = 1; i <= this.settings.maxMatchLength; i++) {
      this.insertNode(r - i);
    }
    this.insertNode(r);

    do {
      if (this.matchLength > len) {
        this.matchLength = len;
      }
      if (this.matchLength <= this.settings.minMatchLength) {
        this.matchLength = 1;
        codeBuf[0] |= mask;
        codeBuf[codeBufPtr++] = this.buffer[r];
      } else {
        codeBuf[codeBufPtr++] = this.matchPosition;
        codeBuf[codeBufPtr++] = ((this.matchPosition >> 4) & 0xf0) |
          (this.matchLength - (this.settings.minMatchLength + 1));
      }
      if ((mask <<= 1) === 0) {
        for (i = 0; i < codeBufPtr; i++) {
          this.writeByte(codeBuf[i]);
        }
        codeBuf[0] = 0;
        codeBufPtr = 1;
        mask = 1;
      }
      lastMatchLength = this.matchLength;
      for (i = 0; i < lastMatchLength && (c = this.readByte()) !== -1; i++) {
        this.deleteNode(s);
        this.buffer[s] = c;
        if (s < this.settings.maxMatchLength - 1) {
          this.buffer[s + this.settings.frameSize] = c;
        }
        s = (s + 1) & (this.settings.frameSize - 1);
        r = (r + 1) & (this.settings.frameSize - 1);
        this.insertNode(r);
      }
      while (i++ < lastMatchLength) {
        this.deleteNode(s);
        s = (s + 1) & (this.settings.frameSize - 1);
        r = (r + 1) & (this.settings.frameSize - 1);
        if (--len !== 0) {
          this.insertNode(r);
        }
      }
    } while (len > 0);

    if (codeBufPtr > 1) {
      for (i = 0; i < codeBufPtr; i++) {
        this.writeByte(codeBuf[i]);
      }
    }
    return Buffer.from(this.output.slice(0, this.outputPos));
  }
}

module.exports = {
  compress: (input) => new LzssCompression(input, true).compress(),
  decompress: (input) => new LzssCompression(input).decompress(),
  LzssCompression,
  LzssSettings
};
