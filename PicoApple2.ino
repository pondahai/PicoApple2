#include <SPI.h>
#include <SD.h>
#include <Arduino.h>
#include <Apple2Core.h>
#include "hardware/gpio.h"
#include "hardware/sync.h"
#include "hardware/clocks.h"
#include "TFT_DMA.h"

#define PIN_DISPLAY_SCK  18
#define PIN_DISPLAY_MOSI 19
#define PIN_DISPLAY_MISO 16
#define PIN_DISPLAY_CS   17
#define PIN_DISPLAY_DC   20
#define PIN_DISPLAY_RST  21
#define PIN_DISPLAY_BL   22
#define SD_SCK           10
#define SD_MOSI          11
#define SD_MISO          12
#define SD_CS            13
#define PIN_JACK_SND     7
#define DATA_OUT_PIN     15
#define LATCH_PIN        14
#define CLOCK_PIN        26
#define DATA_IN_PIN      27

#define BTN_UP    9
#define BTN_DOWN  5
#define BTN_LEFT  8
#define BTN_RIGHT 6
#define BTN_A     2
#define BTN_B     3
#define BTN_MENU  4
#define BTN_ALT   28

TFT_DMA tft_dma(PIN_DISPLAY_CS, PIN_DISPLAY_DC, PIN_DISPLAY_RST, PIN_DISPLAY_MOSI, PIN_DISPLAY_SCK);

const uint16_t palette[16] = {
  0x0000, 0xA010, 0x0014, 0xA01F, 0x0500, 0x8410, 0x001F, 0x051F,
  0xA200, 0xF400, 0x8410, 0xF81F, 0x07E0, 0xFFE0, 0x07FF, 0xFFFF
};

spin_lock_t *res_lock;
static uint16_t scanline_buffers[2][280];
static uint8_t current_buf_idx = 0;

#define KEY_FIFO_SIZE 128
volatile uint8_t g_key_fifo[KEY_FIFO_SIZE];
volatile int g_key_head = 0;
volatile int g_key_tail = 0;
volatile uint8_t g_f_key_event = 0;
volatile bool g_emu_paused = false;
volatile bool g_boot_ready = false; // 新增：開機同步旗標

volatile bool joy_left = false, joy_right = false, joy_up = false, joy_down = false;
volatile bool joy_btn0 = false, joy_btn1 = false;
volatile bool ser_joy_up = false, ser_joy_down = false, ser_joy_left = false, ser_joy_right = false;
volatile bool ser_joy_btn0 = false, ser_joy_btn1 = false;

void pushKey(uint8_t k) {
  int next = (g_key_head + 1) % KEY_FIFO_SIZE;
  if (next != g_key_tail) { g_key_fifo[g_key_head] = k; g_key_head = next; }
}
uint8_t popKey() {
  if (g_key_head == g_key_tail) return 0;
  uint8_t k = g_key_fifo[g_key_tail];
  g_key_tail = (g_key_tail + 1) % KEY_FIFO_SIZE;
  return k;
}
uint8_t peekKey() { return (g_key_head == g_key_tail) ? 0 : g_key_fifo[g_key_tail]; }

const char keymap_base[8][8] = {
  { '1', '3', '5', '7', '9', '-', 206, 204 }, { 'q', 'e', 't', 'u', 'o', '[', 207, '\\' },
  { 'a', 'd', 'g', 'j', 'l', '\'', 205, 208 }, { 'z', 'c', 'b', 'm', '.', 202, 210, 0 },
  { '2', '4', '6', '8', '0', '=', '`', 0 }, { 'w', 'r', 'y', 'i', 'p', ']', 209, '/' },
  { 'x', 'f', 'h', 'k', ';', 203, 212, 214 }, { 's', 'v', 'n', ',', 213, ' ', 211, 0 }
};

extern "C" {
  void apple2_init();
  uint32_t apple2_tick();
  const uint8_t* apple2_get_ram_ptr();
  const uint8_t* apple2_get_char_rom_ptr();
  uint8_t apple2_get_video_mode();
  bool apple2_is_ready_for_key();
  void apple2_handle_key(uint8_t ascii);
  void apple2_set_paddle(uint8_t index, uint8_t value);
  void apple2_set_button(uint8_t index, bool pressed);
  bool apple2_get_disk_motor_status();
  bool apple2_is_track_dirty();
  uint8_t apple2_get_denibblized_track(uint8_t* out_buffer);
  int32_t apple2_needs_disk_reload();
  void apple2_load_track(uint8_t track, const uint8_t* data, uint32_t size);
  void apple2_reset();
  void apple2_warm_reset();
  void arduino_toggle_speaker();
  void Serial_println(const char* msg);
}
void arduino_toggle_speaker() { static bool s = false; s = !s; gpio_put(PIN_JACK_SND, s); }
void Serial_println(const char* msg) { Serial.println(msg); }
File diskFile;
uint8_t track_buffer[4096]; 
uint8_t last_loaded_track = 0;
String g_current_disk_path = "/MASTER.DSK";
bool g_show_menu = false;
String disk_files[20]; 
int disk_file_count = 0;
int selected_file_idx = 0;
volatile uint8_t g_menu_cmd = 0;

// Cross-core SD operation flags
volatile bool req_scan_disks = false;
volatile bool ack_scan_disks = false;
volatile int req_load_disk_idx = -1;


uint8_t keyState[8][8] = {0};
uint8_t lastKeyState[8][8] = {0};
unsigned long lastKeyTime[8][8] = {0};
bool isFnPressed = false;

inline void fastWrite(uint pin, bool val) { gpio_put(pin, val); }
inline bool fastRead(uint pin) { return gpio_get(pin); }
byte myShiftIn(uint8_t dP, uint8_t cP) {
  byte data = 0;
  for (int i = 0; i < 8; i++) { if (fastRead(dP)) { data |= (1 << i); } fastWrite(cP, 1); delayMicroseconds(1); fastWrite(cP, 0); }
  return data;
}

char __flush_debug_buf[256];

void flushDirtyTrack() {
  if (!diskFile) return;
  
  uint32_t irq = spin_lock_blocking(res_lock);
  bool is_dirty = apple2_is_track_dirty();
  if (is_dirty) {
    uint8_t target_track = last_loaded_track; 
    uint32_t offset = (uint32_t)target_track * 4096;
    
    // 讀取當前磁軌數據 (由 Rust 核心進行 Denibblize)
    // 我們需要先從 SD 讀取原始數據作為底層，再覆蓋更新的部分
    if (diskFile.seek(offset)) {
       diskFile.read(track_buffer, 4096);
    }
    
    uint8_t valid_count = apple2_get_denibblized_track(track_buffer);
    
    if (valid_count >= 1) {
      if (diskFile.seek(offset)) {
        size_t written = diskFile.write(track_buffer, 4096); 
        diskFile.flush(); // 強制寫入物理磁區，但不關閉文件
        Serial.printf("[SD] Flush Track %d: %d bytes written\n", target_track, written);
      }
    }
  }
  spin_unlock(res_lock, irq);
}

void loadSingleTrack(uint8_t track) {
  if (!diskFile) return;
  g_emu_paused = true; flushDirtyTrack(); 
  uint32_t offset = (uint32_t)track * 4096;
  uint32_t irq = spin_lock_blocking(res_lock);
  if (diskFile.seek(offset)) {
    if (diskFile.read(track_buffer, 4096) == 4096) {
      apple2_load_track(track, track_buffer, 4096);
      last_loaded_track = track;
    }
  }
  spin_unlock(res_lock, irq);
  g_emu_paused = false; 
}

uint16_t get_text_row_addr(uint8_t row) { return ((row & 0x07) << 7) | ((row & 0x18) * 5) | 0x0400; }
uint16_t get_hires_row_addr(uint8_t row, bool page2) {
  uint16_t base = page2 ? 0x4000 : 0x2000;
  return base | ((row & 0x07) << 10) | ((row & 0x38) << 4) | ((row & 0xC0) >> 1) | ((row & 0xC0) >> 3);
}

void scanDiskFiles() {
  disk_file_count = 0; File root = SD.open("/");
  if (!root) { Serial.println("SD ERROR: Cannot open root directory"); return; }
  Serial.println("SD: Scanning for .DSK files...");
  while (true) {
    File entry = root.openNextFile(); if (!entry) break;
    String name = String(entry.name());
    if (!entry.isDirectory() && (name.endsWith(".DSK") || name.endsWith(".dsk"))) { 
      if (disk_file_count < 20) { 
        disk_files[disk_file_count++] = name; 
        Serial.print("Found: "); Serial.println(name);
      } 
    }
    entry.close();
  }
  root.close();
  Serial.print("SD: Scan complete. Total DSK files: "); Serial.println(disk_file_count);
}

void setup() {
  set_sys_clock_khz(250000, true);
  Serial.begin(115200); delay(2000);
  int lock_num = spin_lock_claim_unused(true);
  res_lock = spin_lock_init(lock_num);
  pinMode(PIN_JACK_SND, OUTPUT);
  uint32_t irq = spin_lock_blocking(res_lock);
  apple2_init();
  spin_unlock(res_lock, irq);
}
void loop() {
  static int esc_state = 0; 
  static char esc_buf[8]; 
  static int esc_idx = 0;
  static int proto_state = 0;
  static uint8_t proto_cmd[3];

  while (Serial.available() > 0) {
    uint8_t sK = Serial.read();
    
    // 非阻塞協議解析 (0x02 開頭)
    if (proto_state > 0) {
      proto_cmd[proto_state-1] = sK;
      if (++proto_state > 3) {
        uint8_t type = proto_cmd[0];
        uint8_t idx = proto_cmd[1];
        uint8_t stat = proto_cmd[2];
        bool b = (stat == 1);
        if (type == 'J') {
          if (b && g_show_menu) { if (idx == 0) g_menu_cmd = 1; else if (idx == 1) g_menu_cmd = 2; else if (idx == 4) g_menu_cmd = 3; else if (idx == 5) g_menu_cmd = 4; }
          if (idx == 0) ser_joy_up = b; else if (idx == 1) ser_joy_down = b; else if (idx == 2) ser_joy_left = b; else if (idx == 3) ser_joy_right = b; else if (idx == 4) ser_joy_btn0 = b; else if (idx == 5) ser_joy_btn1 = b;
        } else if (type == 'K' && stat == 1) {
          if (g_show_menu) { if (idx == 0x0D) g_menu_cmd = 3; else if (idx == 0x1B) g_menu_cmd = 4; }
          if (idx == 112) g_f_key_event = 1; 
          else if (idx == 113) g_f_key_event = 2; 
          else if (idx == 114) g_f_key_event = 3; 
          else pushKey(idx); 
        }
        proto_state = 0;
      }
      continue;
    }

    if (esc_state == 0) {
      if (sK == 0x02) { proto_state = 1; }
      else if (sK == 0x1B) { esc_state = 1; esc_idx = 0; }
      else { 
        static uint8_t last_raw_sk = 0;
        if (sK == 127 || sK == 8) sK = 0x08; 
        else if (sK == '\r') sK = 0x0D; 
        else if (sK == '\n') { if (last_raw_sk == '\r') { last_raw_sk = sK; continue; } sK = 0x0D; }
        else if (sK >= 'a' && sK <= 'z') sK -= 32; 
        pushKey(sK); 
        last_raw_sk = sK;
      }
    } else if (esc_state == 1) {
      if (sK == '[' || sK == 'O') { esc_buf[esc_idx++] = sK; esc_state = 2; } 
      else { if (g_show_menu) g_menu_cmd = 4; esc_state = 0; }
    } else if (esc_state == 2) {
      if (esc_idx < 7) { esc_buf[esc_idx++] = sK; esc_buf[esc_idx] = 0; }
      if ((sK >= 'A' && sK <= 'Z') || sK == '~') {
        if (esc_buf[0] == '[') {
          if (sK == 'A') { if (g_show_menu) g_menu_cmd = 1; else ser_joy_up = true; }
          else if (sK == 'B') { if (g_show_menu) g_menu_cmd = 2; else ser_joy_down = true; }
          else if (strcmp(esc_buf, "[11~") == 0 || strcmp(esc_buf, "OP") == 0) g_f_key_event = 1;
          else if (strcmp(esc_buf, "[12~") == 0 || strcmp(esc_buf, "OQ") == 0) g_f_key_event = 2;
          else if (strcmp(esc_buf, "[13~") == 0 || strcmp(esc_buf, "OR") == 0) g_f_key_event = 3;
        } else if (esc_buf[0] == 'O') {
          if (sK == 'P') g_f_key_event = 1; else if (sK == 'Q') g_f_key_event = 2; else if (sK == 'R') g_f_key_event = 3;
        }
        esc_state = 0;
      }
    }
  }

  if (!g_boot_ready) { yield(); return; }

  // 處理來自 Core 1 的 SD 卡操作請求
  if (req_scan_disks && !ack_scan_disks) {
    scanDiskFiles();
    ack_scan_disks = true;
  }
  if (req_load_disk_idx >= 0) {
    if (disk_file_count > 0 && req_load_disk_idx < disk_file_count) {
      flushDirtyTrack(); if (diskFile) diskFile.close(); 
      g_current_disk_path = "/" + disk_files[req_load_disk_idx];
      diskFile = SD.open(g_current_disk_path, "r+"); if (!diskFile) diskFile = SD.open(g_current_disk_path, "r");
      if (SD.exists("/LASTDISK.TXT")) { SD.remove("/LASTDISK.TXT"); }
      File f = SD.open("/LASTDISK.TXT", FILE_WRITE); if (f) { f.println(g_current_disk_path); f.close(); }
      loadSingleTrack(0);
    }
    req_load_disk_idx = -1;
    g_emu_paused = false;
  }

  if (g_emu_paused && !g_show_menu) { yield(); return; }
  if (g_show_menu) return;
  uint32_t irq_joy = spin_lock_blocking(res_lock);
  apple2_set_paddle(0, (joy_left || ser_joy_left) ? 0 : ((joy_right || ser_joy_right) ? 255 : 128));
  apple2_set_paddle(1, (joy_up || ser_joy_up) ? 0 : ((joy_down || ser_joy_down) ? 255 : 128));
  apple2_set_button(0, joy_btn0 || ser_joy_btn0);
  apple2_set_button(1, joy_btn1 || ser_joy_btn1);
  spin_unlock(res_lock, irq_joy);
  if (peekKey() != 0) {
    uint32_t irq = spin_lock_blocking(res_lock);
    if (apple2_is_ready_for_key()) { apple2_handle_key(popKey()); }
    spin_unlock(res_lock, irq);
  }
  unsigned long start_t = micros();
  static bool last_motor_on = false;
  uint32_t irq_t = spin_lock_blocking(res_lock);
  uint32_t cycles = apple2_tick(); 
  bool motor_on = apple2_get_disk_motor_status();
  int32_t reload_track = apple2_needs_disk_reload();
  spin_unlock(res_lock, irq_t);

  // 延遲寫入邏輯：當馬達從 ON 變為 OFF 時，將髒數據刷入 SD 卡
  if (last_motor_on && !motor_on) {
    flushDirtyTrack();
  }
  last_motor_on = motor_on;

  if (reload_track >= 0) { loadSingleTrack((uint8_t)reload_track); }
  if (!motor_on) {
    unsigned long expected = (unsigned long)((float)cycles / 1.023f);
    unsigned long actual = micros() - start_t;
    if (actual < expected) delayMicroseconds(expected - actual);
  }
}

void setup1() {
  delay(2000); 
  gpio_init(DATA_OUT_PIN); gpio_set_dir(DATA_OUT_PIN, GPIO_OUT);
  gpio_init(LATCH_PIN); gpio_set_dir(LATCH_PIN, GPIO_OUT);
  gpio_init(CLOCK_PIN); gpio_set_dir(CLOCK_PIN, GPIO_OUT);
  gpio_init(DATA_IN_PIN); gpio_set_dir(DATA_IN_PIN, GPIO_IN);
  pinMode(PIN_DISPLAY_BL, OUTPUT); digitalWrite(PIN_DISPLAY_BL, HIGH);
  tft_dma.begin();
  gpio_put(PIN_DISPLAY_RST, 0); delay(100); gpio_put(PIN_DISPLAY_RST, 1); delay(100);
  tft_dma.writeCommand(0x01); delay(150);
  tft_dma.writeCommand(0xCB); tft_dma.writeData(0x39); tft_dma.writeData(0x2C); tft_dma.writeData(0x00); tft_dma.writeData(0x34); tft_dma.writeData(0x02);
  tft_dma.writeCommand(0xCF); tft_dma.writeData(0x00); tft_dma.writeData(0xC1); tft_dma.writeData(0x30);
  tft_dma.writeCommand(0xE8); tft_dma.writeData(0x85); tft_dma.writeData(0x00); tft_dma.writeData(0x78);
  tft_dma.writeCommand(0xEA); tft_dma.writeData(0x00); tft_dma.writeData(0x00);
  tft_dma.writeCommand(0xED); tft_dma.writeData(0x64); tft_dma.writeData(0x03); tft_dma.writeData(0x12); tft_dma.writeData(0x81);
  tft_dma.writeCommand(0xF7); tft_dma.writeData(0x20);
  tft_dma.writeCommand(0xC0); tft_dma.writeData(0x23);
  tft_dma.writeCommand(0xC1); tft_dma.writeData(0x10);
  tft_dma.writeCommand(0xC5); tft_dma.writeData(0x3E); tft_dma.writeData(0x28);
  tft_dma.writeCommand(0xC7); tft_dma.writeData(0x86);
  tft_dma.setRotation(1);
  tft_dma.writeCommand(0x3A); tft_dma.writeData(0x55);
  tft_dma.writeCommand(0xB1); tft_dma.writeData(0x00); tft_dma.writeData(0x18);
  tft_dma.writeCommand(0xB6); tft_dma.writeData(0x08); tft_dma.writeData(0x82); tft_dma.writeData(0x27);
  tft_dma.writeCommand(0x11); delay(150);
  tft_dma.writeCommand(0x29); delay(150);
  spi_set_baudrate(spi0, 30000000);
  tft_dma.fillScreen(palette[0]);
  pinMode(BTN_UP, INPUT_PULLUP); pinMode(BTN_DOWN, INPUT_PULLUP);
  pinMode(BTN_LEFT, INPUT_PULLUP); pinMode(BTN_RIGHT, INPUT_PULLUP);
  pinMode(BTN_A, INPUT_PULLUP); pinMode(BTN_B, INPUT_PULLUP);
  pinMode(BTN_MENU, INPUT_PULLUP); pinMode(BTN_ALT, INPUT_PULLUP);
  SPI1.setRX(SD_MISO); SPI1.setTX(SD_MOSI); SPI1.setSCK(SD_SCK);
  bool sd_ok = false;
  Serial.print("SD: Initializing SPI1 (10MHz)... ");
  // 降低頻率至 10MHz 以提高杜邦線連接的穩定性
  // RP2040 參數順序為 (csPin, frequency, spi)
  for (int i = 0; i < 5; i++) { 
    if (SD.begin(SD_CS, 10000000, SPI1)) { 
      sd_ok = true; 
      Serial.println("OK!");
      break; 
    } 
    Serial.print(".");
    delay(200); 
  }
  
  if (sd_ok) {
    // ... 原有邏輯 ...
    if (SD.exists("/LASTDISK.TXT")) {
      File f = SD.open("/LASTDISK.TXT", FILE_READ);
      if (f) {
        String path = f.readStringUntil('\n');
        path.trim();
        if (path.length() > 0 && SD.exists(path)) { g_current_disk_path = path; }
        f.close();
      }
    }
    diskFile = SD.open(g_current_disk_path, "r+");
    if (!diskFile) diskFile = SD.open(g_current_disk_path, "r");
    if (diskFile) loadSingleTrack(0);
  }
  g_boot_ready = true; // 初始化完成，允許 Core 0 開始模擬
}
void drawString(uint16_t x, uint16_t y, String s, uint16_t color, uint16_t bg) {
  const uint8_t* font = apple2_get_char_rom_ptr(); s.toUpperCase(); 
  for (int i = 0; i < s.length(); i++) { tft_dma.drawChar(x + (i * 7), y, s[i], color, bg, font); }
}

void drawDiskMenu() {
  tft_dma.fillScreen(0x7BEF); tft_dma.drawRect(10, 10, 300, 220, 0xFFFF);
  drawString(30, 30, "SELECT DISK IMAGE:", 0xFFE0, 0x7BEF);
  tft_dma.drawRect(10, 50, 300, 2, 0xFFFF);
  for (int i = 0; i < disk_file_count; i++) {
    uint16_t y = 65 + (i * 12);
    if (i == selected_file_idx) { tft_dma.drawRect(25, y-2, 270, 12, 0x07E0); drawString(30, y, "> " + disk_files[i], 0x0000, 0x07E0); }
    else { drawString(30, y, "  " + disk_files[i], 0xFFFF, 0x7BEF); }
  }
}

void loop1() {
  unsigned long now = millis();
  
  // 將 GPIO 按鍵讀取移到最前面，且不受 40ms 幀率限制
  joy_up = (digitalRead(BTN_UP) == LOW); 
  joy_down = (digitalRead(BTN_DOWN) == LOW);
  joy_left = (digitalRead(BTN_LEFT) == LOW); 
  joy_right = (digitalRead(BTN_RIGHT) == LOW);
  joy_btn0 = (digitalRead(BTN_A) == LOW); 
  joy_btn1 = (digitalRead(BTN_B) == LOW);

  // 修正 Menu 按鍵：增加放開後的保護邏輯，防止重複觸發
  static bool last_menu_p = false; 
  bool menu_p = (digitalRead(BTN_MENU) == LOW);
  if (menu_p && !last_menu_p && !g_show_menu) { 
    g_f_key_event = 3; 
  } 
  last_menu_p = menu_p;

  // 鍵盤矩陣掃描 (維持現狀)
  for (int row = 0; row < 8; row++) {
    byte rS = (1 << row); fastWrite(LATCH_PIN, 0); shiftOut(DATA_OUT_PIN, CLOCK_PIN, MSBFIRST, 0); shiftOut(DATA_OUT_PIN, CLOCK_PIN, MSBFIRST, rS);
    fastWrite(LATCH_PIN, 1); delayMicroseconds(5); fastWrite(LATCH_PIN, 0); delayMicroseconds(1); fastWrite(LATCH_PIN, 1);
    byte colData = myShiftIn(DATA_IN_PIN, CLOCK_PIN);
    for (int col = 0; col < 8; col++) keyState[row][7 - col] = (colData & (1 << col));
  }
  fastWrite(CLOCK_PIN, 1); delayMicroseconds(2); fastWrite(CLOCK_PIN, 0);
  for (int r = 0; r < 8; r++) {
    for (int c = 0; c < 8; c++) {
      bool p = keyState[r][c];
      if (p != lastKeyState[r][c] && (now - lastKeyTime[r][c] > 30)) {
        lastKeyTime[r][c] = now; lastKeyState[r][c] = p;
        if (r == 3 && c == 7) { isFnPressed = p; continue; }
        if (p) {
          if (isFnPressed && keymap_base[r][c] == '1') g_f_key_event = 1;
          else if (isFnPressed && keymap_base[r][c] == '2') g_f_key_event = 2;
          else if (isFnPressed && keymap_base[r][c] == '3') g_f_key_event = 3;
          else if (!g_show_menu) pushKey(keymap_base[r][c]);
          else {
            if (keymap_base[r][c] == (char)209) g_menu_cmd = 1; else if (keymap_base[r][c] == (char)210) g_menu_cmd = 2; else if (keymap_base[r][c] == (char)203) g_menu_cmd = 3;
          }
        }
      }
    }
  }
  if (g_f_key_event == 1) { g_f_key_event = 0; uint32_t irq = spin_lock_blocking(res_lock); apple2_warm_reset(); spin_unlock(res_lock, irq); Serial.println("SYSTEM: Warm Reset"); }
  if (g_f_key_event == 2) { g_f_key_event = 0; uint32_t irq = spin_lock_blocking(res_lock); apple2_reset(); spin_unlock(res_lock, irq); Serial.println("SYSTEM: Cold Reset"); }
  if (g_f_key_event == 3) { 
    g_f_key_event = 0; 
    g_emu_paused = true; 
    req_scan_disks = true; 
    ack_scan_disks = false; 
    g_show_menu = true; 
    tft_dma.fillScreen(0x7BEF); 
    drawString(30, 30, "SCANNING SD...", 0xFFFF, 0x7BEF);
  }
  if (g_show_menu) {
    if (req_scan_disks) {
        if (ack_scan_disks) {
            req_scan_disks = false;
            selected_file_idx = 0;
            if (disk_file_count > 0) drawDiskMenu();
            else { drawString(30, 50, "NO DSK FILES FOUND", 0xF800, 0x7BEF); }
        }
        return;
    }
    static uint32_t last_m_nav = 0;
    bool b_u = joy_up || (g_menu_cmd == 1); bool b_d = joy_down || (g_menu_cmd == 2); bool b_e = joy_btn0 || (g_menu_cmd == 3); bool b_q = joy_btn1 || (g_menu_cmd == 4);
    if (millis() - last_m_nav > 200) {
      if (disk_file_count > 0) {
        if (b_u) { selected_file_idx = (selected_file_idx - 1 + disk_file_count) % disk_file_count; drawDiskMenu(); last_m_nav = millis(); }
        else if (b_d) { selected_file_idx = (selected_file_idx + 1) % disk_file_count; drawDiskMenu(); last_m_nav = millis(); }
        else if (b_e) {
          req_load_disk_idx = selected_file_idx; // 交給 Core 0 處理
          g_show_menu = false; tft_dma.fillScreen(0); last_m_nav = millis();
        }
      }
      if (b_q) { g_show_menu = false; g_emu_paused = false; tft_dma.fillScreen(0); last_m_nav = millis(); }
      g_menu_cmd = 0;
    }
    return;
  }
  static unsigned long last_f = 0;
  if (millis() - last_f > 40) { 
    uint32_t irq = spin_lock_blocking(res_lock);
    const uint8_t* ram = apple2_get_ram_ptr(); const uint8_t* char_rom = apple2_get_char_rom_ptr();
    uint8_t v_m = apple2_get_video_mode(); bool motor_on = apple2_get_disk_motor_status();
    spin_unlock(res_lock, irq);
    static bool last_m_on = false;
    if (motor_on != last_m_on) { tft_dma.drawRect(305, 10, 8, 8, motor_on ? 0xF800 : 0x0000); last_m_on = motor_on; }
    if (ram && char_rom) {
      bool text_m = (v_m & 0x01) != 0; bool mixed_m = (v_m & 0x02) != 0; bool page2 = (v_m & 0x04) != 0; bool hires_m = (v_m & 0x08) != 0;
      bool blink_on = (millis() >> 8) & 0x01;
      tft_dma.startFrame(20, 24, 299, 215);
      for (int y = 0; y < 192; y++) {
        uint16_t* line_ptr = scanline_buffers[current_buf_idx];
        if (text_m || (mixed_m && y >= 160)) {
          uint16_t r_addr = get_text_row_addr(y / 8);
          for (int col = 0; col < 40; col++) {
            uint8_t raw = ram[r_addr + col]; uint8_t c_idx = raw & 0x7F;
            if (raw < 0x80) { if (c_idx < 0x20) c_idx += 0x40; else if (c_idx >= 0x60) c_idx -= 0x40; }
            bool inv = (raw < 0x40) || (raw < 0x80 && blink_on);
            uint8_t font = char_rom[c_idx * 8 + (y % 8)];
            for (int x = 0; x < 7; x++) { bool p = (font & (1 << (6 - x))) != 0; if (inv) p = !p; line_ptr[col * 7 + x] = __builtin_bswap16(p ? 0xFFFF : 0x0000); }
          }
        } else if (hires_m) {
          uint16_t r_addr = get_hires_row_addr(y, page2); bool p_bit = false;
          for (int col = 0; col < 40; col++) {
            uint8_t b = ram[r_addr + col]; bool shift = (b & 0x80) != 0;
            for (int bit = 0; bit < 7; bit++) {
              bool c_bit = (b & (1 << bit)) != 0; bool n_bit = (bit < 6) ? ((b & (1 << (bit + 1))) != 0) : ((col < 39) ? ((ram[r_addr + col + 1] & 0x01) != 0) : false);
              uint16_t color = 0; if (c_bit) { if (p_bit || n_bit) color = 0xFFFF; else { bool even = ((col * 7 + bit) % 2) == 0; color = (!shift) ? (even ? palette[3] : palette[12]) : (even ? palette[6] : palette[9]); } }
              line_ptr[col * 7 + bit] = __builtin_bswap16(color); p_bit = c_bit;
            }
          }
        } else {
          uint16_t r_addr = get_text_row_addr(y / 8); bool lower = (y % 8) < 4;
          for (int col = 0; col < 40; col++) {
            uint8_t val = ram[r_addr + col]; uint8_t c_idx = lower ? (val & 0x0F) : (val >> 4);
            uint16_t color = palette[c_idx & 0x0F]; for (int x = 0; x < 7; x++) { line_ptr[col * 7 + x] = __builtin_bswap16(color); }
          }
        }
        tft_dma.waitTransferDone(); tft_dma.sendScanlineAsync(line_ptr, 280); current_buf_idx = 1 - current_buf_idx;
      }
      tft_dma.waitTransferDone(); gpio_put(PIN_DISPLAY_CS, 1);
    }
    last_f = millis();
  }
}
