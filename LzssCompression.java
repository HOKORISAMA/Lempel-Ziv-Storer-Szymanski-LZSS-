// File: LzssCompression.java
package utility.compression;

import java.io.*;

public class LzssSettings {
    private int frameSize;
    private byte frameFill;
    private int frameInitPos;
    private int maxMatchLength;
    private int minMatchLength;

    public LzssSettings() {
        this.frameSize = 0x1000;
        this.frameFill = 0;
        this.frameInitPos = 0xFEE;
        this.maxMatchLength = 0x12;
        this.minMatchLength = 2;
    }

    // Getters and setters
    public int getFrameSize() { return frameSize; }
    public void setFrameSize(int value) { frameSize = value; }
    public byte getFrameFill() { return frameFill; }
    public void setFrameFill(byte value) { frameFill = value; }
    public int getFrameInitPos() { return frameInitPos; }
    public void setFrameInitPos(int value) { frameInitPos = value; }
    public int getMaxMatchLength() { return maxMatchLength; }
    public void setMaxMatchLength(int value) { maxMatchLength = value; }
    public int getMinMatchLength() { return minMatchLength; }
    public void setMinMatchLength(int value) { minMatchLength = value; }
}

public class LzssCompression implements AutoCloseable {
    private InputStream input;
    private boolean isCompressing;
    private LzssSettings settings;
    private ByteArrayOutputStream output;
    private byte[] buffer;
    private int[] lc;
    private int[] rc;
    private int[] parents;
    private int matchLength;
    private int matchPosition;

    public LzssCompression(InputStream input, boolean isCompressing, LzssSettings settings) {
        this.input = input;
        this.isCompressing = isCompressing;
        this.settings = settings;
        this.output = new ByteArrayOutputStream();
        this.buffer = new byte[settings.getFrameSize() + settings.getMaxMatchLength() - 1];
        
        if (settings.getFrameFill() != 0) {
            for (int j = 0; j < settings.getFrameSize(); ++j) {
                buffer[j] = settings.getFrameFill();
            }
        }
        
        if (isCompressing) {
            initCompress();
        }
    }

    public LzssCompression(InputStream input, boolean isCompressing) {
        this(input, isCompressing, new LzssSettings());
    }

    public LzssCompression(InputStream input) {
        this(input, false);
    }

    private void initCompress() {
        lc = new int[settings.getFrameSize() + 1];
        rc = new int[settings.getFrameSize() + 257];
        parents = new int[settings.getFrameSize() + 1];
        matchLength = 0;
        matchPosition = 0;
    }

    public byte[] decompress() throws IOException {
        if (isCompressing) {
            throw new IllegalStateException("Not in decompression mode");
        }

        int flag = 0;
        int byteRead, distance, length;

        while (true) {
            if (((flag >>= 1) & 256) == 0) {
                if ((byteRead = input.read()) == -1) break;
                flag = byteRead | 0xff00;
            }

            if ((flag & 1) != 0) {
                if ((byteRead = input.read()) == -1) break;
                output.write(byteRead);
                buffer[settings.getFrameInitPos()] = (byte)byteRead;
                settings.setFrameInitPos((settings.getFrameInitPos() + 1) & (settings.getFrameSize() - 1));
            } else {
                if ((distance = input.read()) == -1) break;
                if ((length = input.read()) == -1) break;

                distance |= (length & 0xf0) << 4;
                length = (length & 0x0f) + settings.getMinMatchLength();

                for (int k = 0; k <= length; k++) {
                    byteRead = buffer[(distance + k) & (settings.getFrameSize() - 1)];
                    output.write(byteRead);
                    buffer[settings.getFrameInitPos()] = (byte)byteRead;
                    settings.setFrameInitPos((settings.getFrameInitPos() + 1) & (settings.getFrameSize() - 1));
                }
            }
        }
        return output.toByteArray();
    }

    private void initTree() {
        for (int i = settings.getFrameSize() + 1; i <= settings.getFrameSize() + 256; i++) {
            rc[i] = settings.getFrameSize();
        }
        for (int i = 0; i < settings.getFrameSize(); i++) {
            parents[i] = settings.getFrameSize();
        }
    }

    private void insertNode(int r) {
        int i = 0;
        int p = settings.getFrameSize() + 1 + (buffer[r] & 0xFF);
        int cmp = 1;
        rc[r] = lc[r] = settings.getFrameSize();
        matchLength = 0;

        while (true) {
            if (cmp >= 0) {
                if (rc[p] != settings.getFrameSize()) {
                    p = rc[p];
                } else {
                    rc[p] = r;
                    parents[r] = p;
                    return;
                }
            } else {
                if (lc[p] != settings.getFrameSize()) {
                    p = lc[p];
                } else {
                    lc[p] = r;
                    parents[r] = p;
                    return;
                }
            }

            for (i = 1; i < settings.getMaxMatchLength(); i++) {
                if ((cmp = (buffer[r + i] & 0xFF) - (buffer[p + i] & 0xFF)) != 0) {
                    break;
                }
            }

            if (i > matchLength) {
                matchPosition = p;
                if ((matchLength = i) >= settings.getMaxMatchLength()) {
                    break;
                }
            }
        }
        parents[r] = parents[p];
        lc[r] = lc[p];
        rc[r] = rc[p];
        parents[lc[p]] = r;
        parents[rc[p]] = r;
        if (rc[parents[p]] == p) {
            rc[parents[p]] = r;
        } else {
            lc[parents[p]] = r;
        }
        parents[p] = settings.getFrameSize();
    }

    private void deleteNode(int p) {
        int q;
        if (parents[p] == settings.getFrameSize()) {
            return;
        }
        if (rc[p] == settings.getFrameSize()) {
            q = lc[p];
        } else if (lc[p] == settings.getFrameSize()) {
            q = rc[p];
        } else {
            q = lc[p];
            if (rc[q] != settings.getFrameSize()) {
                do {
                    q = rc[q];
                } while (rc[q] != settings.getFrameSize());
                rc[parents[q]] = lc[q];
                parents[lc[q]] = parents[q];
                lc[q] = lc[p];
                parents[lc[p]] = q;
            }
            rc[q] = rc[p];
            parents[rc[p]] = q;
        }
        parents[q] = parents[p];
        if (rc[parents[p]] == p) {
            rc[parents[p]] = q;
        } else {
            lc[parents[p]] = q;
        }
        parents[p] = settings.getFrameSize();
    }

    public byte[] compress() throws IOException {
        if (!isCompressing) {
            throw new IllegalStateException("Not in compression mode");
        }

        int r = settings.getFrameInitPos();
        int s = 0;
        int len = 0;
        int i, c, lastMatchLength, codeBufPtr;
        byte[] codeBuf = new byte[17];
        byte mask;

        initTree();
        codeBuf[0] = 0;
        codeBufPtr = 1;
        mask = 1;

        for (len = 0; len < settings.getMaxMatchLength() && (c = input.read()) != -1; len++) {
            buffer[r + len] = (byte)c;
        }
        if (len == 0) {
            return new byte[0];
        }
        for (i = 1; i <= settings.getMaxMatchLength(); i++) {
            insertNode(r - i);
        }
        insertNode(r);
        do {
            if (matchLength > len) {
                matchLength = len;
            }
            if (matchLength <= settings.getMinMatchLength()) {
                matchLength = 1;
                codeBuf[0] |= mask;
                codeBuf[codeBufPtr++] = buffer[r];
            } else {
                codeBuf[codeBufPtr++] = (byte)matchPosition;
                codeBuf[codeBufPtr++] = (byte)(((matchPosition >> 4) & 0xf0) | 
                    (matchLength - (settings.getMinMatchLength() + 1)));
            }
            if ((mask <<= 1) == 0) {
                for (i = 0; i < codeBufPtr; i++) {
                    output.write(codeBuf[i]);
                }
                codeBuf[0] = 0;
                codeBufPtr = 1;
                mask = 1;
            }
            lastMatchLength = matchLength;
            for (i = 0; i < lastMatchLength && (c = input.read()) != -1; i++) {
                deleteNode(s);
                buffer[s] = (byte)c;
                if (s < settings.getMaxMatchLength() - 1) {
                    buffer[s + settings.getFrameSize()] = (byte)c;
                }
                s = (s + 1) & (settings.getFrameSize() - 1);
                r = (r + 1) & (settings.getFrameSize() - 1);
                insertNode(r);
            }
            while (i++ < lastMatchLength) {
                deleteNode(s);
                s = (s + 1) & (settings.getFrameSize() - 1);
                r = (r + 1) & (settings.getFrameSize() - 1);
                if (--len != 0) {
                    insertNode(r);
                }
            }
        } while (len > 0);
        if (codeBufPtr > 1) {
            for (i = 0; i < codeBufPtr; i++) {
                output.write(codeBuf[i]);
            }
        }
        return output.toByteArray();
    }

    @Override
    public void close() throws IOException {
        input.close();
        output.close();
    }
}

public class Lzss {
    public static byte[] decompress(byte[] input) throws IOException {
        try (ByteArrayInputStream bis = new ByteArrayInputStream(input);
             LzssCompression lzss = new LzssCompression(bis)) {
            return lzss.decompress();
        }
    }

    public static byte[] compress(byte[] input) throws IOException {
        try (ByteArrayInputStream bis = new ByteArrayInputStream(input);
             LzssCompression lzss = new LzssCompression(bis, true)) {
            return lzss.compress();
        }
    }
}
