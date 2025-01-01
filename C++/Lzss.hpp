#pragma once
#ifndef LZSS_HPP
#define LZSS_HPP

#include <vector>
#include <memory>
#include <cstdint>
#include <istream>
#include <ostream>

namespace Compression {

struct LzssSettings {
    // The size of the sliding window
    int32_t frameSize = 0x1000;
    // The value used to fill the sliding window
    uint8_t frameFill = 0;
    // The initial position of the sliding window
    int32_t frameInitPos = 0xFEE;
    // Maximum match length
    int32_t maxMatchLength = 0x12;
    // Minimum match length
    int32_t minMatchLength = 2;
};

class LzssCompression {
public:
    explicit LzssCompression(std::istream& input, std::ostream& output, bool compress = false, 
                           const LzssSettings& settings = LzssSettings());
    ~LzssCompression();

    void Compress();
    void Decompress();

private:
    void InitCompress();
    void InitTree();
    void InsertNode(int32_t r);
    void DeleteNode(int32_t p);

    std::istream& input_;
    std::ostream& output_;
    std::vector<uint8_t> buffer_;
    bool isCompress_;
    LzssSettings settings_;

    // Tree-related members
    std::vector<int32_t> leftChildren_;    // left children
    std::vector<int32_t> rightChildren_;   // right children
    std::vector<int32_t> parents_;
    int32_t matchLength_;
    int32_t matchPosition_;
};

// Helper functions that use memory streams for convenience
std::vector<uint8_t> CompressData(const std::vector<uint8_t>& input);
std::vector<uint8_t> DecompressData(const std::vector<uint8_t>& input);

} // namespace Compression

#endif // LZSS_HPP
