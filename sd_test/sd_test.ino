
#include <SPI.h>
#include <SD.h>

// --- 完全複刻主程式的腳位定義 ---
#define PIN_DISPLAY_SCK  18
#define PIN_DISPLAY_MOSI 19
#define PIN_DISPLAY_MISO 16
#define PIN_DISPLAY_CS   17

#define SD_SCK           10
#define SD_MOSI          11
#define SD_MISO          12
#define SD_CS            13

void run_sd_test() {
  Serial.println("\n--- [START] Pico SD Sync-With-Main Test ---");

  // 1. 模擬主程式的 setup1 流程
  Serial.println("[DEBUG] Step 1: Initializing SPI0 (Display pins)...");
  SPI.setRX(PIN_DISPLAY_MISO); 
  SPI.setTX(PIN_DISPLAY_MOSI); 
  SPI.setSCK(PIN_DISPLAY_SCK);
  SPI.begin();

  Serial.println("[DEBUG] Step 2: Configuring SPI1 (SD pins)...");
  SPI1.setRX(SD_MISO); 
  SPI1.setTX(SD_MOSI); 
  SPI1.setSCK(SD_SCK);

  // 2. 呼叫與主程式完全一致的 SD.begin
  Serial.println("[DEBUG] Step 3: Calling SD.begin(SD_CS, SPI1)...");
  if (!SD.begin(SD_CS, SPI1)) {
    Serial.println("[ERROR] SD.begin failed! Hardware not responding.");
    return;
  }
  Serial.println("[OK] SD Initialized (Sync Mode).");

  const char* testFileName = "/TEST_SYNC.DSK";
  
  // 1. 寫入測試 (完全模擬主程式的 flushDirtyTrack 邏輯)
  Serial.println("[1/3] Testing 'r+' mode write-back...");
  
  // 先建立檔案 (如果不存在)
  if (!SD.exists(testFileName)) {
    File setupF = SD.open(testFileName, FILE_WRITE);
    if (setupF) {
      uint8_t dummy[512]; memset(dummy, 0x00, 512);
      for(int i=0; i<280; i++) setupF.write(dummy, 512);
      setupF.close();
      Serial.println("  Initial 140KB file created.");
    }
  }

  // 模擬主程式載入方式：用 "r+" 開啟
  File diskFile = SD.open(testFileName, "r+");
  if (diskFile) {
    uint8_t track_buffer[64];
    memset(track_buffer, 0x55, 64);
    
    if (diskFile.seek(0)) {
      size_t written = diskFile.write(track_buffer, 64);
      diskFile.flush();
      
      if (written == 64) {
        Serial.println("[OK] Overwrite write successful.");
        
        // 校驗
        diskFile.seek(0);
        uint8_t verify[64];
        diskFile.read(verify, 64);
        if (verify[0] == 0x55) {
          Serial.println("[OK] Read-back Verification PASSED.");
        } else {
          Serial.println("[FAIL] Verify FAILED! Data mismatch.");
        }
      } else {
        Serial.print("[FAIL] Write failed! Written: "); Serial.println(written);
      }
    }
    diskFile.close();
  } else {
    Serial.println("[ERROR] Could not open file in 'r+' mode.");
  }

  Serial.println("--- [END] Test Completed ---");
}

void setup() {
  Serial.begin(115200);
  delay(3000); 
}

void loop() {
  run_sd_test();
  delay(10000); // 延長間隔以便觀察
}
