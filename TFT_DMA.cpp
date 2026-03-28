#include "TFT_DMA.h"
#include "hardware/gpio.h"

TFT_DMA::TFT_DMA(uint cs, uint dc, uint rst, uint mosi, uint sck) 
    : _cs(cs), _dc(dc), _rst(rst), _mosi(mosi), _sck(sck), _dma_chan(-1) {}

void TFT_DMA::begin() {
    gpio_init(_cs); gpio_set_dir(_cs, GPIO_OUT); gpio_put(_cs, 1);
    gpio_init(_dc); gpio_set_dir(_dc, GPIO_OUT); gpio_put(_dc, 0);
    gpio_init(_rst); gpio_set_dir(_rst, GPIO_OUT); gpio_put(_rst, 1);

    spi_init(_spi, 20000000);
    gpio_set_function(_mosi, GPIO_FUNC_SPI);
    gpio_set_function(_sck, GPIO_FUNC_SPI);
    
    _dma_chan = dma_claim_unused_channel(true);
    dma_channel_config c = dma_channel_get_default_config(_dma_chan);
    channel_config_set_transfer_data_size(&c, DMA_SIZE_8);
    channel_config_set_read_increment(&c, true);
    channel_config_set_write_increment(&c, false);
    channel_config_set_dreq(&c, spi_get_index(_spi) == 0 ? DREQ_SPI0_TX : DREQ_SPI1_TX);
    dma_channel_configure(_dma_chan, &c, &spi_get_hw(_spi)->dr, NULL, 0, false);
}

void TFT_DMA::writeCommand(uint8_t cmd) {
    waitTransferDone();
    gpio_put(_dc, 0); gpio_put(_cs, 0);
    spi_write_blocking(_spi, &cmd, 1);
    gpio_put(_cs, 1);
}

void TFT_DMA::writeData(uint8_t data) {
    waitTransferDone();
    gpio_put(_dc, 1); gpio_put(_cs, 0);
    spi_write_blocking(_spi, &data, 1);
    gpio_put(_cs, 1);
}

void TFT_DMA::startFrame(uint16_t x0, uint16_t y0, uint16_t x1, uint16_t y1) {
    writeCommand(0x2A); 
    uint8_t xdata[] = {(uint8_t)(x0 >> 8), (uint8_t)(x0 & 0xFF), (uint8_t)(x1 >> 8), (uint8_t)(x1 & 0xFF)};
    gpio_put(_dc, 1); gpio_put(_cs, 0); spi_write_blocking(_spi, xdata, 4); gpio_put(_cs, 1);

    writeCommand(0x2B);
    uint8_t ydata[] = {(uint8_t)(y0 >> 8), (uint8_t)(y0 & 0xFF), (uint8_t)(y1 >> 8), (uint8_t)(y1 & 0xFF)};
    gpio_put(_dc, 1); gpio_put(_cs, 0); spi_write_blocking(_spi, ydata, 4); gpio_put(_cs, 1);

    waitTransferDone();
    gpio_put(_dc, 0); gpio_put(_cs, 0);
    uint8_t cmd = 0x2C;
    spi_write_blocking(_spi, &cmd, 1);
    gpio_put(_dc, 1); // 切換為 Data 模式準備 DMA
}

void TFT_DMA::sendScanlineAsync(const uint16_t* buffer, uint16_t length) {
    dma_channel_set_read_addr(_dma_chan, buffer, false);
    dma_channel_set_trans_count(_dma_chan, length * 2, true);
}

void TFT_DMA::waitTransferDone() {
    if (dma_channel_is_busy(_dma_chan)) {
        dma_channel_wait_for_finish_blocking(_dma_chan);
    }
}

void TFT_DMA::setRotation(uint8_t r) {
    writeCommand(0x36);
    switch (r % 4) {
        case 0: writeData(0x48); break;
        case 1: writeData(0x28); break;
        case 2: writeData(0x88); break;
        case 3: writeData(0xE8); break;
    }
}

void TFT_DMA::fillScreen(uint16_t color) {
    drawRect(0, 0, 320, 240, color);
}

void TFT_DMA::drawRect(uint16_t x, uint16_t y, uint16_t w, uint16_t h, uint16_t color) {
    startFrame(x, y, x + w - 1, y + h - 1);
    uint8_t buf[2] = { (uint8_t)(color >> 8), (uint8_t)(color & 0xFF) };
    for (uint32_t i = 0; i < (uint32_t)w * h; i++) {
        spi_write_blocking(_spi, buf, 2);
    }
    gpio_put(_cs, 1);
}

void TFT_DMA::drawChar(uint16_t x, uint16_t y, char c, uint16_t color, uint16_t bg, const uint8_t* font_rom) {
    if (!font_rom) return;
    uint8_t char_idx = (uint8_t)c;
    if (char_idx >= 32 && char_idx <= 63) char_idx += 64;
    else if (char_idx >= 96) char_idx -= 32;

    startFrame(x, y, x + 6, y + 7);
    uint8_t c_buf[2] = { (uint8_t)(color >> 8), (uint8_t)(color & 0xFF) };
    uint8_t b_buf[2] = { (uint8_t)(bg >> 8), (uint8_t)(bg & 0xFF) };

    for (int row = 0; row < 8; row++) {
        uint8_t bits = font_rom[char_idx * 8 + row];
        for (int col = 0; col < 7; col++) {
            bool pixel = (bits & (1 << (6 - col))) != 0;
            spi_write_blocking(_spi, pixel ? c_buf : b_buf, 2);
        }
    }
    gpio_put(_cs, 1);
}
