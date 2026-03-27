// ==========================================================================
//   Pico Apple II Emulator - Accurate Hires Color & Corrected Joystick
// ==========================================================================

#include <Adafruit_GFX.h>
#include <Adafruit_ILI9341.h>
#include <SPI.h>
#include <SD.h>
#include <Arduino.h>
#include <Apple2Core.h>
#include "hardware/gpio.h"
#include "hardware/sync.h"
#include "hardware/clocks.h"

// --- GPIO DEFINITIONS ---
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

Adafruit_ILI9341 tft = Adafruit_ILI9341(PIN_DISPLAY_CS, PIN_DISPLAY_DC, PIN_DISPLAY_RST);

// Apple II Palette (RGB565)
// 0:Black, 1:Magenta, 2:DarkBlue, 3:Purple, 4:DarkGreen, 5:Grey1, 6:MediumBlue, 7:LightBlue,
// 8:Brown, 9:Orange, 10:Grey2, 11:Pink, 12:LightGreen, 13:Yellow, 14:Aqua, 15:White
const uint16_t palette[16] = {
  0x0000, 0xA010, 0x0014, 0xA01F, 0x0500, 0x8410, 0x001F, 0x051F,
  0xA200, 0xF400, 0x8410, 0xF81F, 0x07E0, 0xFFE0, 0x07FF, 0xFFFF
};

spin_lock_t *res_lock;
static uint16_t line_buffer[280];

#define KEY_FIFO_SIZE 32
volatile uint8_t g_key_fifo[KEY_FIFO_SIZE];
volatile int g_key_head = 0;
volatile int g_key_tail = 0;
volatile uint8_t g_f_key_event = 0;
volatile bool g_emu_paused = false;

volatile bool joy_left = false, joy_right = false, joy_up = false, joy_down = false;
volatile bool joy_btn0 = false, joy_btn1 = false;
// [新增] 專門給 Serial 使用的狀態，避免與實體掃描衝突
volatile bool ser_joy_up = false, ser_joy_down = false, ser_joy_left = false, ser_joy_right = false;
volatile bool ser_joy_btn0 = false, ser_joy_btn1 = false;
volatile uint32_t joy_auto_release_ms[6] = {0}; // 0:U, 1:D, 2:L, 3:R, 4:B0, 5:B1

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
uint8_t peekKey() {
  if (g_key_head == g_key_tail) return 0;
  return g_key_fifo[g_key_tail];
}

const char keymap_base[8][8] = {
  { '1', '3', '5', '7', '9', '-', 206, 204 }, { 'q', 'e', 't', 'u', 'o', '[', 207, '\\' },
  { 'a', 'd', 'g', 'j', 'l', '\'', 205, 208 }, { 'z', 'c', 'b', 'm', '.', 202, 210, 0 },
  { '2', '4', '6', '8', '0', '=', '`', 0 }, { 'w', 'r', 'y', 'i', 'p', ']', 209, '/' },
  { 'x', 'f', 'h', 'k', ';', 203, 212, 214 }, { 's', 'v', 'n', ',', 213, ' ', 211, 0 }
};
const char keymap_shifted[8][8] = {
  { '!', '#', '%', '&', '(', '_', 206, 204 }, { 'Q', 'E', 'T', 'U', 'O', '{', 207, '|' },
  { 'A', 'D', 'G', 'J', 'L', '"', 205, 208 }, { 'Z', 'C', 'B', 'M', '>', 202, 210, 0 },
  { '@', '$', '^', '*', ')', '+', '~', 0 }, { 'W', 'R', 'Y', 'I', 'P', '}', 209, '?' },
  { 'X', 'F', 'H', 'K', ':', 203, 212, 214 }, { 'S', 'V', 'N', '<', 213, ' ', 211, 0 }
};

extern "C" void apple2_init();
extern "C" uint32_t apple2_tick();
extern "C" const uint8_t* apple2_get_ram_ptr();
extern "C" const uint8_t* apple2_get_char_rom_ptr();
extern "C" const uint8_t* apple2_get_disk_rom_ptr();
extern "C" uint8_t apple2_get_video_mode();
extern "C" bool apple2_is_ready_for_key();
extern "C" void apple2_handle_key(uint8_t ascii);
extern "C" void apple2_set_paddle(uint8_t index, uint8_t value);
extern "C" void apple2_set_button(uint8_t index, bool pressed);
extern "C" bool apple2_get_disk_motor_status();
extern "C" uint32_t apple2_get_disk_byte_index();
extern "C" bool apple2_is_track_dirty();
extern "C" uint8_t apple2_get_denibblized_track(uint8_t* out_buffer);
extern "C" int32_t apple2_needs_disk_reload();
extern "C" void apple2_load_track(uint8_t track, const uint8_t* data, uint32_t size);
extern "C" void apple2_reset();
extern "C" void apple2_warm_reset();

extern "C" void arduino_toggle_speaker() {
  static bool s = false; s = !s; gpio_put(PIN_JACK_SND, s);
}
extern "C" void Serial_println(const char* msg) { Serial.println(msg); }

File diskFile;
uint8_t track_buffer[4096]; 
uint8_t current_disk_track = 0;
uint8_t last_loaded_track = 0;
String g_current_disk_path = "/MASTER.DSK";

bool g_show_menu = false;
String disk_files[20]; 
int disk_file_count = 0;
int selected_file_idx = 0;

#define BTN_UP    9
#define BTN_DOWN  5
#define BTN_LEFT  8
#define BTN_RIGHT 6
#define BTN_A     2
#define BTN_B     3
#define BTN_MENU  4
#define BTN_ALT   28

void flushDirtyTrack() {
  if (!diskFile) return;
  
  uint32_t irq = spin_lock_blocking(res_lock);
  if (apple2_is_track_dirty()) {
    uint8_t target_track = last_loaded_track; 
    uint32_t offset = (uint32_t)target_track * 4096;
    
    // --- [核心修正：Read-Modify-Write] ---
    // 在解碼前，先讀取 SD 卡上該磁軌的原始資料
    // 這樣解碼器沒認出來的扇區，就會保留 SD 卡原本正確的內容，不會被垃圾覆蓋
    if (diskFile.seek(offset)) {
      diskFile.read(track_buffer, 4096);
    }

    Serial.print("SD: Syncing DIRTY track "); Serial.print(target_track); Serial.println("...");
    uint8_t valid_count = apple2_get_denibblized_track(track_buffer);
    spin_unlock(res_lock, irq);
    
    if (valid_count >= 1) {
      Serial.print("SD: Writing Track "); Serial.print(target_track); 
      Serial.print(" (Updated "); Serial.print(valid_count); Serial.println(" sectors)");
      
      if (diskFile.seek(offset)) {
        size_t written = diskFile.write(track_buffer, 4096);
        diskFile.flush();
        diskFile.close(); // 先關閉，確保 Metadata 與資料同步

        if (written == 4096) {
          // --- [重新開啟並進行強力驗證] ---
          diskFile = SD.open(g_current_disk_path, "r+");
          if (diskFile && diskFile.seek(offset)) {
            uint8_t verify_buf[64];
            diskFile.read(verify_buf, 64);
            bool verify_ok = true;
            for (int i = 0; i < 64; i++) {
              if (verify_buf[i] != track_buffer[i]) { 
                verify_ok = false; 
                Serial.print("SD VERIFY ERROR at byte "); Serial.print(i);
                Serial.print(": Exp "); Serial.print(track_buffer[i], HEX);
                Serial.print(" Got "); Serial.println(verify_buf[i], HEX);
                break; 
              }
            }

            if (verify_ok) {
              Serial.println("SD: Write & Physical Verify [SUCCESS].");
            } else {
              Serial.println("SD ERROR: Data on disk does NOT match buffer!");
            }
          } else {
            Serial.println("SD ERROR: Re-open/Seek failed for verify!");
          }
        } else {
          Serial.print("SD ERROR: Write mismatch! Got "); Serial.println(written);
        }
      } else {
        Serial.print("SD ERROR: Seek failed at offset "); Serial.println(offset);
      }
    } else {
      Serial.println("SD: Skip write, 0 sectors decoded.");
    }
  } else {
    spin_unlock(res_lock, irq);
  }
}

void loadSingleTrack(uint8_t track) {
  if (!diskFile) return;
  g_emu_paused = true; flushDirtyTrack(); 
  uint32_t offset = (uint32_t)track * 4096;
  if (diskFile.seek(offset)) {
    if (diskFile.read(track_buffer, 4096) == 4096) {
      uint32_t irq = spin_lock_blocking(res_lock);
      apple2_load_track(track, track_buffer, 4096);
      current_disk_track = track;
      last_loaded_track = track; // 更新目前載入的磁軌號
      spin_unlock(res_lock, irq);
    }
  }
  g_emu_paused = false; 
}

uint16_t get_text_row_addr(uint8_t row) {
  return ((row & 0x07) << 7) | ((row & 0x18) * 5) | 0x0400;
}
uint16_t get_hires_row_addr(uint8_t row, bool page2) {
  uint16_t base = page2 ? 0x4000 : 0x2000;
  return base | ((row & 0x07) << 10) | ((row & 0x38) << 4) | ((row & 0xC0) >> 1) | ((row & 0xC0) >> 3);
}

void setup() {
  set_sys_clock_khz(250000, true);
  Serial.begin(115200);
  delay(1000);
  int lock_num = spin_lock_claim_unused(true);
  res_lock = spin_lock_init(lock_num);
  pinMode(PIN_JACK_SND, OUTPUT);
  uint32_t irq = spin_lock_blocking(res_lock);
  apple2_init();

  // --- [DEBUG: Verify Slot 6 ROM] ---
  const uint8_t* disk_rom = apple2_get_disk_rom_ptr();
  Serial.print("DEBUG: Slot 6 ROM Signature (at $C600): ");
  if (disk_rom) {
    for (int i = 0; i < 16; i++) {
      if (disk_rom[i] < 0x10) Serial.print("0");
      Serial.print(disk_rom[i], HEX);
      Serial.print(" ");
    }
    Serial.println();
  } else {
    Serial.println("FAILED TO GET DISK ROM POINTER!");
  }

  spin_unlock(res_lock, irq);
}

volatile bool g_monitor_mode = false;

volatile uint8_t g_menu_cmd = 0; // 0:None, 1:Up, 2:Down, 3:Enter, 4:Esc

void loop() {
  static unsigned long last_perf_ms = 0;
  static uint32_t total_cycles_sec = 0;
  static bool motor_on = false;
  if (g_emu_paused && !g_monitor_mode && !g_show_menu) { return; }

  // --- [Terminal Keyboard & ANSI Parser] ---
  static int esc_state = 0;
  static char esc_buf[8];
  static int esc_idx = 0;

  while (Serial.available() > 0) {
    uint8_t sK = Serial.read();

    if (g_monitor_mode) {
      if (sK == 0x1B) { // ESC to exit monitor
        g_monitor_mode = false; g_emu_paused = false;
        Serial.println("\n[Monitor Mode] Exited.");
        continue;
      }
      Serial.print("Raw Hex: 0x"); if (sK < 0x10) Serial.print("0");
      Serial.print(sK, HEX); Serial.print(" ('"); Serial.print((char)sK); Serial.println("')");
      continue;
    }

    if (esc_state == 0) {
      if (sK == 0x1B) { 
        esc_state = 1; esc_idx = 0; 
      }
      else if (sK == 0x02) { // [STX] - Real-time Protocol (Press/Release)
        uint32_t timeout = micros() + 1000;
        while (Serial.available() < 3 && micros() < timeout); 
        if (Serial.available() >= 3) {
          uint8_t type = Serial.read();
          uint8_t idx  = Serial.read();
          uint8_t stat = Serial.read();
          if (type == 'J') { // Joystick (0:U, 1:D, 2:L, 3:R, 4:B0, 5:B1)
            bool b = (stat == 1);
            if (b && g_show_menu) {
              if (idx == 0) g_menu_cmd = 1;      // Up
              else if (idx == 1) g_menu_cmd = 2; // Down
              else if (idx == 4) g_menu_cmd = 3; // Enter (PB0)
              else if (idx == 5) g_menu_cmd = 4; // Esc (PB1)
            }
            if (idx == 0) { ser_joy_up = b; joy_auto_release_ms[0] = 0; }
            else if (idx == 1) { ser_joy_down = b; joy_auto_release_ms[1] = 0; }
            else if (idx == 2) { ser_joy_left = b; joy_auto_release_ms[2] = 0; }
            else if (idx == 3) { ser_joy_right = b; joy_auto_release_ms[3] = 0; }
            else if (idx == 4) { ser_joy_btn0 = b; joy_auto_release_ms[4] = 0; }
            else if (idx == 5) { ser_joy_btn1 = b; joy_auto_release_ms[5] = 0; }
          } else if (type == 'K') { // Keyboard ASCII
            if (stat == 1) {
              if (g_show_menu) {
                if (idx == 0x0D) g_menu_cmd = 3;      // Enter
                else if (idx == 0x1B) g_menu_cmd = 4; // Esc
                else if (idx == 'Q' || idx == 'X') g_menu_cmd = 4;
              }
              pushKey(idx); 
            }
          }
        }
      }
      else if (sK == 0x0B) { // Ctrl+K (0x0B) to enter Monitor Mode
        g_monitor_mode = true; g_emu_paused = true;
        Serial.println("\n[Monitor Mode] Started. Press ESC to exit.");
        Serial.println("Press any key to see its Hex code:");
      }
      else {
        // Normal character processing
        if (sK == 127 || sK == 8) sK = 0x08; 
        else if (sK == '\r' || sK == '\n') {
          sK = 0x0D; 
          if (g_show_menu) g_menu_cmd = 3; // Enter in menu
        }
        else if (sK >= 'a' && sK <= 'z') sK -= 32; 

        if (g_show_menu && (sK == 'Q' || sK == 'X')) g_menu_cmd = 4;
        
        // Control characters (0x01-0x1A for CTRL+A..Z) are kept as is
        pushKey(sK);
      }
    } else if (esc_state == 1) { // Expecting '[' or 'O'
      if (sK == '[' || sK == 'O') { esc_buf[esc_idx++] = sK; esc_state = 2; }
      else { 
        if (g_show_menu) g_menu_cmd = 4; // Standalone ESC exits menu
        esc_state = 0; 
      }
    } else if (esc_state == 2) { // Gathering parameters
      if (esc_idx < 7) esc_buf[esc_idx++] = sK;
      // End of sequence detection
      if ((sK >= 'A' && sK <= 'Z') || sK == '~') {
        // Parse results
        if (esc_buf[0] == '[') {
          if (sK == 'A') { if (g_show_menu) g_menu_cmd = 1; else { ser_joy_up = true; joy_auto_release_ms[0] = millis() + 150; } }
          else if (sK == 'B') { if (g_show_menu) g_menu_cmd = 2; else { ser_joy_down = true; joy_auto_release_ms[1] = millis() + 150; } }
          else if (sK == 'C') { ser_joy_right = true; joy_auto_release_ms[3] = millis() + 150; }
          else if (sK == 'D') { ser_joy_left = true; joy_auto_release_ms[2] = millis() + 150; }
          else if (String(esc_buf).indexOf("11~") >= 0) g_f_key_event = 1; // F1
          else if (String(esc_buf).indexOf("12~") >= 0) g_f_key_event = 2; // F2
          else if (String(esc_buf).indexOf("13~") >= 0) g_f_key_event = 3; // F3
          else if (String(esc_buf).indexOf("15~") >= 0) g_f_key_event = 4; // F4
          else if (sK == '~') {
            if (String(esc_buf).indexOf("5~") >= 0) { ser_joy_btn0 = true; joy_auto_release_ms[4] = millis() + 200; } // PGUP
            else if (String(esc_buf).indexOf("6~") >= 0) { ser_joy_btn1 = true; joy_auto_release_ms[5] = millis() + 200; } // PGDN
          }
        } else if (esc_buf[0] == 'O') { // SS3 sequences
          if (sK == 'P') g_f_key_event = 1; // F1
          else if (sK == 'Q') g_f_key_event = 2; // F2
          else if (sK == 'R') g_f_key_event = 3; // F3
          else if (sK == 'S') g_f_key_event = 4; // F4
        }
        esc_state = 0;
      }
    }
  }
  
  // --- [Joystick Auto-Release Logic for Legacy Terminal] ---
  uint32_t now_ms = millis();
  if (joy_auto_release_ms[0] && now_ms > joy_auto_release_ms[0]) { ser_joy_up = false; joy_auto_release_ms[0] = 0; }
  if (joy_auto_release_ms[1] && now_ms > joy_auto_release_ms[1]) { ser_joy_down = false; joy_auto_release_ms[1] = 0; }
  if (joy_auto_release_ms[2] && now_ms > joy_auto_release_ms[2]) { ser_joy_left = false; joy_auto_release_ms[2] = 0; }
  if (joy_auto_release_ms[3] && now_ms > joy_auto_release_ms[3]) { ser_joy_right = false; joy_auto_release_ms[3] = 0; }
  if (joy_auto_release_ms[4] && now_ms > joy_auto_release_ms[4]) { ser_joy_btn0 = false; joy_auto_release_ms[4] = 0; }
  if (joy_auto_release_ms[5] && now_ms > joy_auto_release_ms[5]) { ser_joy_btn1 = false; joy_auto_release_ms[5] = 0; }
  
  if (g_monitor_mode) return; // In monitor mode, skip emulation
  if (g_show_menu) return;    // In menu mode, skip emulation loop

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
  
  static int m_cnt = 0;
  if (m_cnt++ > 10) {
    uint32_t irq_m = spin_lock_blocking(res_lock);
    motor_on = apple2_get_disk_motor_status();
    uint32_t b_idx = apple2_get_disk_byte_index();
    spin_unlock(res_lock, irq_m);
    m_cnt = 0;

    static uint32_t last_disk_debug = 0;
    if (motor_on && (millis() - last_disk_debug > 1000)) {
        Serial.print("DISK: Motor ON, Byte Index: "); Serial.println(b_idx);
        last_disk_debug = millis();
    }
  }

  unsigned long start_t = micros();
  uint32_t irq = spin_lock_blocking(res_lock);
  uint32_t cycles = apple2_tick(); 
  spin_unlock(res_lock, irq);
  total_cycles_sec += cycles;

  if (!motor_on) {
    unsigned long expected = (unsigned long)((float)cycles / 1.023f);
    unsigned long actual = micros() - start_t;
    if (actual < expected) delayMicroseconds(expected - actual);
  }
  
  if (millis() - last_perf_ms > 1000) {
    total_cycles_sec = 0;
    last_perf_ms = millis();
  }
}

void setup1() {
  delay(6000); 
  gpio_init(DATA_OUT_PIN); gpio_set_dir(DATA_OUT_PIN, GPIO_OUT);
  gpio_init(LATCH_PIN); gpio_set_dir(LATCH_PIN, GPIO_OUT);
  gpio_init(CLOCK_PIN); gpio_set_dir(CLOCK_PIN, GPIO_OUT);
  gpio_init(DATA_IN_PIN); gpio_set_dir(DATA_IN_PIN, GPIO_IN);
  pinMode(PIN_DISPLAY_BL, OUTPUT); digitalWrite(PIN_DISPLAY_BL, HIGH);
  pinMode(PIN_DISPLAY_CS, OUTPUT); digitalWrite(PIN_DISPLAY_CS, HIGH);
  pinMode(BTN_UP, INPUT_PULLUP); pinMode(BTN_DOWN, INPUT_PULLUP);
  pinMode(BTN_LEFT, INPUT_PULLUP); pinMode(BTN_RIGHT, INPUT_PULLUP);
  pinMode(BTN_A, INPUT_PULLUP); pinMode(BTN_B, INPUT_PULLUP);
  pinMode(BTN_MENU, INPUT_PULLUP); pinMode(BTN_ALT, INPUT_PULLUP);
  SPI.setRX(PIN_DISPLAY_MISO); SPI.setTX(PIN_DISPLAY_MOSI); SPI.setSCK(PIN_DISPLAY_SCK);
  SPI.begin();
  tft.begin(63000000); tft.setRotation(1); tft.fillScreen(ILI9341_BLACK);
  SPI1.setRX(SD_MISO); SPI1.setTX(SD_MOSI); SPI1.setSCK(SD_SCK);
  if (SD.begin(SD_CS, SPI1)) {
    // --- [新增：讀取最後一次使用的磁碟紀錄] ---
    String lastDisk = "/MASTER.DSK";
    if (SD.exists("/LASTDISK.TXT")) {
      File f = SD.open("/LASTDISK.TXT", "r");
      if (f) {
        String s = f.readStringUntil('\n');
        s.trim();
        if (s.length() > 0) lastDisk = s;
        f.close();
        Serial.print("SD: Found last disk record: "); Serial.println(lastDisk);
      }
    }

    g_current_disk_path = lastDisk;
    diskFile = SD.open(g_current_disk_path, "r+"); 
    if (diskFile) {
      Serial.print("SD: "); Serial.print(g_current_disk_path); Serial.println(" opened in r+ mode.");
    } else {
      Serial.print("SD: "); Serial.print(g_current_disk_path); Serial.println(" r+ FAILED, trying r mode...");
      diskFile = SD.open(g_current_disk_path, "r");
    }
    if (diskFile) { loadSingleTrack(0); }
    else {
      // 如果連最後紀錄都開不起來，最後回退到 MASTER.DSK
      if (g_current_disk_path != "/MASTER.DSK") {
        g_current_disk_path = "/MASTER.DSK";
        diskFile = SD.open(g_current_disk_path, "r");
        if (diskFile) loadSingleTrack(0);
      }
    }
  }
}

uint8_t keyState[8][8] = {0};
uint8_t lastKeyState[8][8] = {0};
unsigned long lastKeyTime[8][8] = {0};
#define DEBOUNCE_DELAY 30
bool isShiftPressed = false;
bool isFnPressed = false;
bool isCtrlPressed = false;

inline void fastWrite(uint pin, bool val) { gpio_put(pin, val); }
inline bool fastRead(uint pin) { return gpio_get(pin); }

byte myShiftIn(uint8_t dataPin, uint8_t clockPin) {
  byte data = 0;
  for (int i = 0; i < 8; i++) {
    if (fastRead(dataPin)) { data |= (1 << i); }
    fastWrite(clockPin, 1); delayMicroseconds(1); fastWrite(clockPin, 0);
  }
  return data;
}

void scanDiskFiles() {
  disk_file_count = 0;
  File root = SD.open("/");
  while (true) {
    File entry = root.openNextFile();
    if (!entry) break;
    String name = String(entry.name());
    if (!entry.isDirectory() && (name.endsWith(".DSK") || name.endsWith(".dsk"))) {
      if (disk_file_count < 20) { disk_files[disk_file_count++] = name; }
    }
    entry.close();
  }
  root.close();
}

void drawDiskMenu() {
  tft.fillScreen(ILI9341_DARKGREY);
  tft.drawRect(10, 10, 300, 220, ILI9341_WHITE);
  tft.setCursor(20, 20); tft.setTextColor(ILI9341_YELLOW); tft.setTextSize(2);
  tft.println("Select Disk Image:");
  tft.drawLine(10, 45, 310, 45, ILI9341_WHITE);
  tft.setTextSize(1);
  for (int i = 0; i < disk_file_count; i++) {
    tft.setCursor(30, 55 + (i * 10));
    if (i == selected_file_idx) { tft.setTextColor(ILI9341_BLACK, ILI9341_GREEN); tft.print("> " + disk_files[i] + " <"); }
    else { tft.setTextColor(ILI9341_WHITE, ILI9341_DARKGREY); tft.print("  " + disk_files[i]); }
  }
}

void loop1() {
  unsigned long now = millis();
  char menu_key = 0;

  // --- [掃描實體按鈕 (GPIO)] ---
  joy_up = (digitalRead(BTN_UP) == LOW);
  joy_down = (digitalRead(BTN_DOWN) == LOW);
  joy_left = (digitalRead(BTN_LEFT) == LOW);
  joy_right = (digitalRead(BTN_RIGHT) == LOW);
  joy_btn0 = (digitalRead(BTN_A) == LOW);
  joy_btn1 = (digitalRead(BTN_B) == LOW);

  static bool last_menu_p = false;
  bool menu_p = (digitalRead(BTN_MENU) == LOW);
  if (menu_p && !last_menu_p) { g_f_key_event = 3; } // MENU -> Disk Menu
  last_menu_p = menu_p;

  for (int row = 0; row < 8; row++) {
    byte rowScanData = (1 << row);
    fastWrite(LATCH_PIN, 0);
    shiftOut(DATA_OUT_PIN, CLOCK_PIN, MSBFIRST, 0);
    shiftOut(DATA_OUT_PIN, CLOCK_PIN, MSBFIRST, rowScanData);
    fastWrite(LATCH_PIN, 1); delayMicroseconds(5);
    fastWrite(LATCH_PIN, 0); delayMicroseconds(1);
    fastWrite(LATCH_PIN, 1);
    byte colData = myShiftIn(DATA_IN_PIN, CLOCK_PIN);
    for (int col = 0; col < 8; col++) keyState[row][7 - col] = (colData & (1 << col));
  }
  fastWrite(CLOCK_PIN, 1); delayMicroseconds(2); fastWrite(CLOCK_PIN, 0);

  for (int r = 0; r < 8; r++) {
    for (int c = 0; c < 8; c++) {
      bool p = keyState[r][c];
      if (p != lastKeyState[r][c] && (now - lastKeyTime[r][c] > DEBOUNCE_DELAY)) {
        lastKeyTime[r][c] = now; lastKeyState[r][c] = p;
        char bK = keymap_base[r][c];
        
        // 搖桿對應 (包含自動歸零)
        bool is_joy_key = false;
        if (bK == (char)211) { joy_left = p; is_joy_key = true; }
        else if (bK == (char)212) { joy_right = p; is_joy_key = true; }
        else if (bK == (char)209) { joy_up = p; is_joy_key = true; }
        else if (bK == (char)210) { joy_down = p; is_joy_key = true; }
        else if (bK == (char)213) { joy_btn0 = p; is_joy_key = true; }
        else if (bK == (char)214) { joy_btn1 = p; is_joy_key = true; }
        
        // 只有在「非選單模式」下才攔截搖桿按鍵，避免它們產生鍵盤字元
        // 在選單模式中，必須放行讓它們作為選單導航使用
        if (is_joy_key && !g_show_menu) continue; 

        if (r == 3 && c == 7) { isFnPressed = p; continue; }
        if (bK == (char)202) { isShiftPressed = p; continue; }
        if (bK == (char)205) { isCtrlPressed = p; continue; }
        
        if (p) {
          if (isFnPressed && bK >= '1' && bK <= '9') { g_f_key_event = bK - '0'; }
          else if (!g_show_menu) {
            char finalK = isShiftPressed ? keymap_shifted[r][c] : bK;
            uint8_t aK = (uint8_t)finalK;
            if (finalK == (char)203) aK = 0x0D; 
            else if (finalK == (char)204) aK = 0x08; 
            else if (finalK == (char)207) aK = 0x1B;
            
            // --- [處理 CTRL 組合鍵] ---
            if (isCtrlPressed) {
              if (aK >= 'a' && aK <= 'z') aK -= 96;      // a-z -> 0x01-0x1A
              else if (aK >= 'A' && aK <= 'Z') aK -= 64; // A-Z -> 0x01-0x1A
              else if (aK == '@') aK = 0x00;
              else if (aK == '[') aK = 0x1B;
              else if (aK == '\\') aK = 0x1C;
              else if (aK == ']') aK = 0x1D;
              else if (aK == '^') aK = 0x1E;
              else if (aK == '_') aK = 0x1F;
            } else if (aK >= 'a' && aK <= 'z') {
              aK -= 32; // Normal uppercase
            }
            
            pushKey(aK);
          } else { menu_key = bK; }
        }
      }
    }
  }

  uint32_t irq_sys = spin_lock_blocking(res_lock);
  int32_t reload_track = apple2_needs_disk_reload();
  bool cur_m_s = apple2_get_disk_motor_status();
  spin_unlock(res_lock, irq_sys);

  if (reload_track >= 0) { loadSingleTrack((uint8_t)reload_track); }

  if (g_f_key_event == 1) { g_f_key_event = 0; uint32_t ir = spin_lock_blocking(res_lock); apple2_warm_reset(); spin_unlock(res_lock, ir); }
  if (g_f_key_event == 2) { 
    g_f_key_event = 0; uint32_t ir = spin_lock_blocking(res_lock); apple2_reset(); spin_unlock(res_lock, ir); 
    tft.fillScreen(ILI9341_BLACK); loadSingleTrack(0);
  }
  if (g_f_key_event == 3) { g_f_key_event = 0; scanDiskFiles(); if (disk_file_count > 0) { g_emu_paused = true; g_show_menu = true; selected_file_idx = 0; drawDiskMenu(); } }

  if (g_show_menu) {
    static unsigned long last_m_ms = 0;
    bool b_up = (digitalRead(BTN_UP) == LOW) || (menu_key == (char)209) || (g_menu_cmd == 1);
    bool b_down = (digitalRead(BTN_DOWN) == LOW) || (menu_key == (char)210) || (g_menu_cmd == 2);
    bool b_a = (digitalRead(BTN_A) == LOW) || (menu_key == (char)203) || (g_menu_cmd == 3);
    bool b_b = (digitalRead(BTN_B) == LOW) || (menu_key == (char)207) || (g_menu_cmd == 4);
    if (millis() - last_m_ms > 200) {
      if (b_up) { selected_file_idx = (selected_file_idx - 1 + disk_file_count) % disk_file_count; drawDiskMenu(); last_m_ms = millis(); g_menu_cmd = 0; }
      else if (b_down) { selected_file_idx = (selected_file_idx + 1) % disk_file_count; drawDiskMenu(); last_m_ms = millis(); g_menu_cmd = 0; }
      else if (b_a) {
        g_menu_cmd = 0;
        if (disk_file_count > 0) {
          g_emu_paused = true; flushDirtyTrack(); 
          if (diskFile) diskFile.close(); 
          g_current_disk_path = "/" + disk_files[selected_file_idx];
          diskFile = SD.open(g_current_disk_path, "r+"); 
          if (diskFile) {
            Serial.print("SD: Switched to "); Serial.print(g_current_disk_path); Serial.println(" (r+ mode).");
            // --- [新增：儲存最後一次使用的磁碟紀錄 (安全覆寫)] ---
            if (SD.exists("/LASTDISK.TXT")) { SD.remove("/LASTDISK.TXT"); }
            File f = SD.open("/LASTDISK.TXT", FILE_WRITE);
            if (f) {
              f.println(g_current_disk_path);
              f.close();
              Serial.println("SD: Saved last disk record to /LASTDISK.TXT");
            }
          }
 else {
            Serial.print("SD: Switched to "); Serial.print(g_current_disk_path); Serial.println(" (r mode).");
            diskFile = SD.open(g_current_disk_path, "r");
          }
          loadSingleTrack(0);
        }
        g_show_menu = false; g_emu_paused = false; tft.fillScreen(ILI9341_BLACK); last_m_ms = millis();
      }
      else if (b_b) { g_menu_cmd = 0; g_show_menu = false; g_emu_paused = false; tft.fillScreen(ILI9341_BLACK); last_m_ms = millis(); }
    }
    return;
  }

  static unsigned long last_f = 0;
  static bool prev_m_s = false;
  if (prev_m_s && !cur_m_s) { g_emu_paused = true; flushDirtyTrack(); g_emu_paused = false; }
  prev_m_s = cur_m_s;

  if (millis() - last_f > 40) { 
    tft.fillCircle(305, 15, 5, cur_m_s ? ILI9341_RED : ILI9341_BLACK); 
    uint32_t irq = spin_lock_blocking(res_lock);
    const uint8_t* ram = apple2_get_ram_ptr();
    const uint8_t* char_rom = apple2_get_char_rom_ptr();
    uint8_t v_mode = apple2_get_video_mode();
    spin_unlock(res_lock, irq);
    
    bool blink_on = (millis() >> 8) & 0x01; // 約 256ms 閃爍週期

    if (ram && char_rom) {
      bool text_m = (v_mode & 0x01) != 0; bool mixed_m = (v_mode & 0x02) != 0; bool page2 = (v_mode & 0x04) != 0; bool hires_m = (v_mode & 0x08) != 0;
      tft.startWrite(); tft.setAddrWindow(20, 24, 280, 192);
      for (int y = 0; y < 192; y++) {
        if (text_m || (mixed_m && y >= 160)) {
          uint16_t row_addr = get_text_row_addr(y / 8);
          for (int col = 0; col < 40; col++) {
            uint8_t raw_char = ram[row_addr + col];
            uint8_t char_idx = raw_char & 0x7F;

            // Apple II 字符映射修正 (處理 Inverse/Flashing 區域)
            if (raw_char < 0x80) {
                if (char_idx < 0x20) char_idx += 0x40;      // $00-$1F -> $40-$5F (@.._)
                else if (char_idx >= 0x60) char_idx -= 0x40; // $60-$7F -> $20-$3F (Space..?)
            }

            bool invert = false;
            if (raw_char < 0x40) invert = true;            // Inverse Mode
            else if (raw_char < 0x80) invert = blink_on;    // Flashing Mode (游標所在地)

            uint8_t font_row = char_rom[char_idx * 8 + (y % 8)];
            for (int x = 0; x < 7; x++) { 
                bool pixel = (font_row & (1 << (6 - x))) != 0;
                if (invert) pixel = !pixel;
                line_buffer[col * 7 + x] = pixel ? 0xFFFF : 0x0000; 
            }
          }
        } else if (hires_m) {
          uint16_t row_addr = get_hires_row_addr(y, page2);
          bool p_bit = false;
          for (int col = 0; col < 40; col++) {
            uint8_t b = ram[row_addr + col];
            bool shift = (b & 0x80) != 0;
            for (int bit = 0; bit < 7; bit++) {
              bool c_bit = (b & (1 << bit)) != 0;
              bool n_bit = (bit < 6) ? ((b & (1 << (bit + 1))) != 0) : ((col < 39) ? ((ram[row_addr + col + 1] & 0x01) != 0) : false);
              uint16_t color = 0;
              if (c_bit) {
                if (p_bit || n_bit) color = 0xFFFF;
                else {
                  bool even = ((col * 7 + bit) % 2) == 0;
                  if (!shift) color = even ? palette[3] : palette[12]; // Purple/Green
                  else color = even ? palette[6] : palette[9]; // Blue/Orange
                }
              }
              line_buffer[col * 7 + bit] = color; p_bit = c_bit;
            }
          }
        } else {
          // --- [補全 Lo-Res (LGR) 渲染邏輯] ---
          uint16_t row_addr = get_text_row_addr(y / 8);
          // 每一個 Byte 代表上下兩個 4x7 的色塊。y % 8 < 4 畫上方(低4位)，>= 4 畫下方(高4位)
          bool is_lower_nibble = (y % 8) < 4;
          for (int col = 0; col < 40; col++) {
            uint8_t val = ram[row_addr + col];
            uint8_t color_idx = is_lower_nibble ? (val & 0x0F) : (val >> 4);
            uint16_t color = palette[color_idx & 0x0F];
            for (int x = 0; x < 7; x++) {
              line_buffer[col * 7 + x] = color;
            }
          }
        }
        tft.writePixels(line_buffer, 280); 
      }
      tft.endWrite();
    }
    last_f = millis();
  }
}
