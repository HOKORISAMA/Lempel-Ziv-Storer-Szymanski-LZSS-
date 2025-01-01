#include "Lzss.hpp"
#include <stdexcept>
#include <sstream>

namespace Compression {

LzssCompression::LzssCompression(std::istream& input, std::ostream& output, bool compress, const LzssSettings& settings)
    : input_(input)
    , output_(output)
    , isCompress_(compress)
    , settings_(settings)
    , buffer_(settings.frameSize + settings.maxMatchLength - 1)
{
    if (settings.frameFill != 0) {
        std::fill(buffer_.begin(), buffer_.begin() + settings.frameSize, settings.frameFill);
    }
    
    if (isCompress_) {
        InitCompress();
    }
}

LzssCompression::~LzssCompression() = default;

void LzssCompression::InitCompress() {
    leftChildren_.resize(settings_.frameSize + 1);
    rightChildren_.resize(settings_.frameSize + 257);
    parents_.resize(settings_.frameSize + 1);
    matchLength_ = 0;
    matchPosition_ = 0;
}

void LzssCompression::InitTree() {
    for (int32_t i = settings_.frameSize + 1; i <= settings_.frameSize + 256; i++) {
        rightChildren_[i] = settings_.frameSize;
    }
    for (int32_t i = 0; i < settings_.frameSize; i++) {
        parents_[i] = settings_.frameSize;
    }
}

void LzssCompression::InsertNode(int32_t r) {
    int32_t i = 0;
    int32_t p = settings_.frameSize + 1 + buffer_[r];
    int32_t cmp = 1;
    rightChildren_[r] = leftChildren_[r] = settings_.frameSize;
    matchLength_ = 0;

    while (true) {
        if (cmp >= 0) {
            if (rightChildren_[p] != settings_.frameSize) {
                p = rightChildren_[p];
            } else {
                rightChildren_[p] = r;
                parents_[r] = p;
                return;
            }
        } else {
            if (leftChildren_[p] != settings_.frameSize) {
                p = leftChildren_[p];
            } else {
                leftChildren_[p] = r;
                parents_[r] = p;
                return;
            }
        }

        for (i = 1; i < settings_.maxMatchLength; i++) {
            if ((cmp = buffer_[r + i] - buffer_[p + i]) != 0) {
                break;
            }
        }

        if (i > matchLength_) {
            matchPosition_ = p;
            if ((matchLength_ = i) >= settings_.maxMatchLength) {
                break;
            }
        }
    }
    parents_[r] = parents_[p];
    leftChildren_[r] = leftChildren_[p];
    rightChildren_[r] = rightChildren_[p];
    parents_[leftChildren_[p]] = r;
    parents_[rightChildren_[p]] = r;
    if (rightChildren_[parents_[p]] == p) {
        rightChildren_[parents_[p]] = r;
    } else {
        leftChildren_[parents_[p]] = r;
    }
    parents_[p] = settings_.frameSize;
}

void LzssCompression::DeleteNode(int32_t p) {
    int32_t q;
    
    if (parents_[p] == settings_.frameSize) {
        return;
    }
    
    if (rightChildren_[p] == settings_.frameSize) {
        q = leftChildren_[p];
    } else if (leftChildren_[p] == settings_.frameSize) {
        q = rightChildren_[p];
    } else {
        q = leftChildren_[p];
        if (rightChildren_[q] != settings_.frameSize) {
            do {
                q = rightChildren_[q];
            } while (rightChildren_[q] != settings_.frameSize);
            rightChildren_[parents_[q]] = leftChildren_[q];
            parents_[leftChildren_[q]] = parents_[q];
            leftChildren_[q] = leftChildren_[p];
            parents_[leftChildren_[p]] = q;
        }
        rightChildren_[q] = rightChildren_[p];
        parents_[rightChildren_[p]] = q;
    }
    parents_[q] = parents_[p];
    if (rightChildren_[parents_[p]] == p) {
        rightChildren_[parents_[p]] = q;
    } else {
        leftChildren_[parents_[p]] = q;
    }
    parents_[p] = settings_.frameSize;
}

void LzssCompression::Decompress() {
    if (isCompress_) {
        throw std::runtime_error("Not in decompression mode");
    }

    uint32_t flag = 0;
    int32_t byteRead, distance, length;
    
    while (true) {
        if (((flag >>= 1) & 256) == 0) {
            byteRead = input_.get();
            if (byteRead == std::char_traits<char>::eof()) break;
            flag = static_cast<uint8_t>(byteRead) | 0xff00;
        }

        if ((flag & 1) != 0) {
            byteRead = input_.get();
            if (byteRead == std::char_traits<char>::eof()) break;
            output_.put(static_cast<char>(byteRead));
            buffer_[settings_.frameInitPos++] = static_cast<uint8_t>(byteRead);
            settings_.frameInitPos &= settings_.frameSize - 1;
        } else {
            distance = input_.get();
            if (distance == std::char_traits<char>::eof()) break;
            length = input_.get();
            if (length == std::char_traits<char>::eof()) break;

            distance |= (length & 0xf0) << 4;
            length = (length & 0x0f) + settings_.minMatchLength;

            for (int32_t k = 0; k <= length; k++) {
                byteRead = buffer_[(distance + k) & (settings_.frameSize - 1)];
                output_.put(static_cast<char>(byteRead));
                buffer_[settings_.frameInitPos++] = static_cast<uint8_t>(byteRead);
                settings_.frameInitPos &= settings_.frameSize - 1;
            }
        }
    }
}

void LzssCompression::Compress() {
    if (!isCompress_) {
        throw std::runtime_error("Not in compression mode");
    }

    int32_t r = settings_.frameInitPos;
    int32_t s = 0;
    int32_t len = 0;
    int32_t i, c, lastMatchLength;
    std::vector<uint8_t> codeBuf(17);
    size_t codeBufPtr = 1;
    uint8_t mask = 1;

    InitTree();
    codeBuf[0] = 0;

    // Read initial bytes
    for (len = 0; len < settings_.maxMatchLength; len++) {
        c = input_.get();
        if (c == std::char_traits<char>::eof()) break;
        buffer_[r + len] = static_cast<uint8_t>(c);
    }
    
    if (len == 0) {
        return;
    }

    for (i = 1; i <= settings_.maxMatchLength; i++) {
        InsertNode(r - i);
    }
    InsertNode(r);

    do {
        if (matchLength_ > len) {
            matchLength_ = len;
        }
        
        if (matchLength_ <= settings_.minMatchLength) {
            matchLength_ = 1;
            codeBuf[0] |= mask;
            codeBuf[codeBufPtr++] = buffer_[r];
        } else {
            codeBuf[codeBufPtr++] = static_cast<uint8_t>(matchPosition_);
            codeBuf[codeBufPtr++] = static_cast<uint8_t>(
                ((matchPosition_ >> 4) & 0xf0) | 
                (matchLength_ - (settings_.minMatchLength + 1))
            );
        }

        if ((mask <<= 1) == 0) {
            for (i = 0; i < codeBufPtr; i++) {
                output_.put(static_cast<char>(codeBuf[i]));
            }
            codeBuf[0] = 0;
            codeBufPtr = 1;
            mask = 1;
        }

        lastMatchLength = matchLength_;
        for (i = 0; i < lastMatchLength; i++) {
            c = input_.get();
            if (c == std::char_traits<char>::eof()) break;
            DeleteNode(s);
            buffer_[s] = static_cast<uint8_t>(c);
            if (s < settings_.maxMatchLength - 1) {
                buffer_[s + settings_.frameSize] = static_cast<uint8_t>(c);
            }
            s = (s + 1) & (settings_.frameSize - 1);
            r = (r + 1) & (settings_.frameSize - 1);
            InsertNode(r);
        }
        
        while (i++ < lastMatchLength) {
            DeleteNode(s);
            s = (s + 1) & (settings_.frameSize - 1);
            r = (r + 1) & (settings_.frameSize - 1);
            if (--len != 0) {
                InsertNode(r);
            }
        }
    } while (len > 0);

    if (codeBufPtr > 1) {
        for (i = 0; i < codeBufPtr; i++) {
            output_.put(static_cast<char>(codeBuf[i]));
        }
    }
}

// Helper functions that use memory streams
std::vector<uint8_t> CompressData(const std::vector<uint8_t>& input) {
    std::stringstream inputStream;
    std::stringstream outputStream;
    inputStream.write(reinterpret_cast<const char*>(input.data()), input.size());
    
    LzssCompression lzss(inputStream, outputStream, true);
    lzss.Compress();
    
    std::string result = outputStream.str();
    return std::vector<uint8_t>(result.begin(), result.end());
}

std::vector<uint8_t> DecompressData(const std::vector<uint8_t>& input) {
    std::stringstream inputStream;
    std::stringstream outputStream;
    inputStream.write(reinterpret_cast<const char*>(input.data()), input.size());
    
    LzssCompression lzss(inputStream, outputStream, false);
    lzss.Decompress();
    
    std::string result = outputStream.str();
    return std::vector<uint8_t>(result.begin(), result.end());
}

} // namespace Compression
