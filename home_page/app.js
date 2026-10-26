/**
 * ============================================================================
 * LaduinoPLC (by pksofttech.org) — Official Landing Page Configuration
 * แก้ไขข้อมูลลิงก์ดาวน์โหลด, เลขพร้อมเพย์, รูป QR Code และลิงก์รับบริจาคได้ที่บล็อกนี้
 * ============================================================================
 */
const SITE_CONFIG = {
  version: "v0.1.0",
  githubRepoUrl: "https://github.com/pksofttech/uPLC_STUDIO",
  releasesUrl: "#download",
  windowsDownloadUrl:
    "https://github.com/pksofttech/uPLC_STUDIO/releases/latest/download/laduino-plc-v0.1.0-x86_64-pc-windows-msvc.zip",
  linuxDownloadUrl:
    "https://github.com/pksofttech/uPLC_STUDIO/releases/latest/download/laduino-plc-v0.1.0-x86_64-unknown-linux-gnu.tar.gz",
  firmwareUf2Url:
    "https://github.com/pksofttech/uPLC_STUDIO/releases/latest/download/uplc-rp2040-embassy.uf2",

  // ข้อมูลรับบริจาค / Support & Donate
  promptPayQrImage: "assets/promptpay-qr.svg",
  promptPayNameTh: "บรรณพต นุ่มนาค",
  promptPayNameEn: "บรรณพต นุ่มนาค",
  promptPayId: "3660600255351",
  githubSponsorsUrl: "https://github.com/sponsors/pksofttech",
  buyMeACoffeeUrl: "https://buymeacoffee.com/pksofttech",
  reportEmail: "pksofttech@gmail.com",

  // ข้อมูลระบบวิเคราะห์สถิติผู้เยี่ยมชม (Visitor & Download Analytics)
  counterNamespace: "pksofttech-uplc-studio",
  githubApiRepo: "pksofttech/uPLC_STUDIO",
  googleAnalyticsId: "" // ใส่รหัส GA4 เช่น "G-XXXXXXXXXX" หากต้องการเชื่อมต่อ Google Analytics เพิ่มเติม
};

/**
 * Bilingual Dictionary (TH / EN)
 */
const I18N = {
  th: {
    nav_features: "ฟีเจอร์เด่น",
    nav_getting_started: "เริ่มต้นใช้งาน",
    nav_hardware: "ฮาร์ดแวร์ & พิน",
    nav_examples: "ตัวอย่างวงจร",
    nav_roadmap: "แผนพัฒนา",
    nav_download: "ดาวน์โหลด",
    nav_report: "แจ้งปัญหา",
    nav_donate: "☕ สนับสนุนผู้พัฒนา",

    hero_badge: "IEC 61131-3 Ladder IDE & RP2040 Micro-VM",
    status_prerelease_short: "สถานะ: ปรับปรุงก่อนปล่อยออกอย่างเป็นทางการ",
    status_prerelease_tag: "PRE-RELEASE",
    status_prerelease_desc: "ปรับปรุงก่อนปล่อยออกอย่างเป็นทางการ — รุ่นทดสอบสำหรับตรวจสอบความเสถียรและฟีเจอร์ก่อนเปิดตัวเวอร์ชันสมบูรณ์",
    hero_title_1: "เขียนวงจร PLC Ladder",
    hero_title_2: "จำลองและแฟลชลงบอร์ดจริงในคลิกเดียว",
    hero_desc:
      "LaduinoPLC โดย pksofttech.org — โปรแกรมออกแบบวงจร Ladder Diagram มาตรฐาน IEC 61131-3 พร้อมระบบจำลองการทำงานแบบเรียลไทม์ (Real-Time Simulator), แปลงโค้ด Instruction List (.il) อัตโนมัติ และรันบนไมโครคอนโทรลเลอร์ Raspberry Pi Pico (RP2040) ด้วย 32-bit Micro-VM ความเร็วสูง",
    btn_download_now: "⬇ ดาวน์โหลดฟรี (GitHub Releases)",
    btn_quickstart: "🚀 คู่มือเริ่มต้นใช้งาน",
    btn_support_dev: "💖 ร่วมบริจาคสนับสนุน",

    stat_1_val: "100% IEC",
    stat_1_lbl: "มาตรฐาน IEC 61131-3 IL & Ladder",
    stat_2_val: "32-bit VM",
    stat_2_lbl: "RP2040 Embassy Async Runtime",
    stat_3_val: "Real-Time",
    stat_3_lbl: "Online Monitoring & Force I/O",

    sim_window_title: "LaduinoPLC — Live Interactive Preview (ลองคลิกปุ่มด้านล่าง!)",
    sim_comment: "// Network 01: วงจรควบคุมมอเตอร์ Start / Stop พร้อมหน้าสัมผัสค้างสถานะ (Self-Holding)",
    sim_hint: "💡 คลิกปุ่ม %IX0.0 (START) หรือ %IX0.1 (STOP) เพื่อทดลองจำลองกระแสไหลบนหน้าเว็บ:",

    feat_tag: "KEY CAPABILITIES",
    feat_title: "ครบเครื่องเรื่อง PLC สมัยใหม่ในโปรแกรมเดียว",
    feat_subtitle: "ออกแบบมาเพื่องานการเรียนการสอน งานต้นแบบอัตโนมัติ และงานควบคุมอุตสาหกรรมขนาดเล็กด้วยไมโครคอนโทรลเลอร์",

    f1_title: "IEC 61131-3 Ladder & IL Editor",
    f1_desc: "วาดวงจรด้วยเมาส์และคีย์บอร์ดความเร็วสูง รองรับ Contact (NO/NC/P/N), Coil (Normal/Negated/Set/Reset), Branch ขนาน, Multi-line Network Comment และแปลงไป-กลับไฟล์ .il มาตรฐานได้ทันที",
    f2_title: "Function Blocks ระดับอุตสาหกรรม",
    f2_desc: "รองรับคำสั่งขั้นสูงครบครันทั้ง Timer (TON, TOF, TP), Counter (CTU, CTD, CTUD), PID Controller, Linear SCALE, LIMIT, MATH, MOVE, BITWISE, Shift Register, ADC และ UART",
    f3_title: "Real-Time Simulator & Online Monitor",
    f3_desc: "ทดสอบโปรแกรมได้ทันทีโดยไม่ต้องมีบอร์ดจริง หรือเชื่อมต่อบอร์ด RP2040 ผ่าน USB เพื่อดูเส้นทางการไหลของกระแสไฟ (Live Conduction), ตาราง Watch Table และสั่ง Force I/O แบบสดๆ",
    f4_title: "RP2040 32-bit Micro-VM & RETAIN Flash",
    f4_desc: "คอมไพล์วงจรเป็น Bytecode ขนาดกะทัดรัดพร้อมตรวจเช็ค CRC16 บันทึกลงหน่วยความจำ Flash บนบอร์ด RP2040 และรองรับตัวแปรจำค่าเมื่อไฟดับ (RETAIN: M64..M95, D96..D127)",
    f5_title: "Modbus RTU Master (RS-485)",
    f5_desc: "เชื่อมต่อเซนเซอร์วัดอุณหภูมิ มิเตอร์ไฟฟ้า และอินเวอร์เตอร์ (VFD) ผ่านพอร์ต RS-485 ด้วยบล็อกคำสั่ง MB_MASTER (Function 03, 04, 06) พร้อมระบบจัดการ Timeout อัตโนมัติ",
    f6_title: "1-Click USB BOOTSEL Flasher & Auto-Update",
    f6_desc: "มีระบบแฟลชเฟิร์มแวร์ .uf2 ลงบอร์ด Raspberry Pi Pico ในตัวผ่านโหมด USB BOOTSEL และมีระบบตรวจสอบอัปเดตเวอร์ชันโปรแกรมใหม่ผ่านออนไลน์",

    start_tag: "GETTING STARTED",
    start_title: "เริ่มต้นใช้งาน LaduinoPLC ใน 4 ขั้นตอน",
    start_subtitle: "เมื่อเปิดโปรแกรมขึ้นมา ระบบจะสร้างหน้า New Ladder ที่ว่างเปล่าให้พร้อมเริ่มเขียนวงจรได้ทันที",

    s1_title: "ดาวน์โหลดและเปิดโปรแกรม",
    s1_desc: "ดาวน์โหลดไฟล์สำหรับ Windows หรือ Linux จากหน้า GitHub Releases แตกไฟล์แล้วเปิดโปรแกรม laduino-plc ได้ทันทีโดยไม่ต้องติดตั้งไลบรารีเสริม",
    s1_box: "เปิดโปรแกรมปุ๊บ → เข้าสู่หน้า New Ladder (1 Network ว่าง) พร้อมใช้งานทันที หรือเลือกเมนู Examples เพื่อดูตัวอย่างกว่า 18 โปรเจกต์",

    s2_title: "วางอุปกรณ์และตั้งค่า IEC Address",
    s2_desc: "คลิกเลือก Contact, Coil หรือ Function Block จากแถบด้านซ้ายแล้ววางลงบนเส้น Rung ดับเบิลคลิกที่ตัวอุปกรณ์เพื่อกำหนด Address (%IX0.0, %QX0.0, T0) และตั้งชื่อ Symbol",
    s2_box: "คีย์ลัด: P = ต่อขนาน (Parallel) | S = ต่ออนุกรม (Series) | Enter = ตั้งค่าอุปกรณ์ | Ctrl+F = ค้นหาและแทนที่",

    s3_title: "จำลองการทำงาน (Offline Simulator)",
    s3_desc: "สลับโหมดเป็น Simulator แล้วกดปุ่ม ▶ Run (F5) คุณสามารถคลิกที่หน้าสัมผัส Input บนผังวงจรเพื่อสลับสถานะ ON/OFF และดูค่าตัวแปรในแท็บ Watch ด้านขวาได้ทันที",
    s3_box: "คีย์ลัดจำลอง: F5 = Run / Stop | Shift+F5 = Reset | F6 = Single Step (รันทีละ 1 สแกน)",

    s4_title: "แฟลชเฟิร์มแวร์ & อัปโหลดลงบอร์ด RP2040",
    s4_desc: "กดปุ่ม ⚡ Update to PLC เลือกแท็บ Firmware Flasher เพื่อแฟลชไฟล์ .uf2 ลงบอร์ด Pico ครั้งแรก จากนั้นกด ⬆ Upload to PLC เพื่อส่งโปรแกรม Ladder ลงบอร์ดจริง",
    s4_box: "กด F7 (Online Monitor) เพื่อดูสถานะกระแสไหลจริงจากบอร์ด RP2040 พร้อมระบบเช็ค CRC16 ตรงกับหน้าจอ",

    hw_tag: "HARDWARE & MEMORY MAP",
    hw_title: "ตารางพินและหน่วยความจำมาตรฐาน (RP2040 Micro-VM)",
    hw_subtitle: "การแมปพอร์ตมาตรฐานสำหรับบอร์ด Raspberry Pi Pico (RP2040) พร้อมระบบปรับเปลี่ยนพิน GPIO & RS-485 ผ่านโปรแกรมแบบ Runtime โดยไม่ต้อง Build Firmware ใหม่",
    th_region: "หมวดหมู่ (Region)",
    th_iec: "IEC 61131-3 Address",
    th_alias: "PLC Alias",
    th_pin: "พินฮาร์ดแวร์ RP2040 (ปรับเปลี่ยนผ่าน UI ได้)",
    th_desc: "รายละเอียดการใช้งาน",
    hw_r1_desc: "อินพุตดิจิทัล 8 ช่อง (ปรับพิน, Pull-Up/Down และ Active-Low/High ผ่านโปรแกรมได้ทันที)",
    hw_r2_desc: "เอาต์พุตดิจิทัล 8 ช่อง (Y0..Y5 ค่าเริ่มต้น GP15,14,7,6,2,3 ปรับพินผ่าน UI บันทึกลง Flash 0x1FD000 ได้)",
    hw_r_led_desc: "ไฟสถานะ PLC บนบอร์ด (เลือกโหมด 1Hz Heartbeat หรือติดค้าง และเปลี่ยนพินได้แบบ Runtime)",
    hw_r3_desc: "รีเลย์ช่วยภายในทั่วไป (เคลียร์ค่าเมื่อเริ่มรันใหม่)",
    hw_r4_desc: "รีเลย์ช่วยชนิดจำค่าเมื่อไฟดับ บันทึกลง RP2040 Flash อัตโนมัติ",
    hw_r5_desc: "รีจิสเตอร์ข้อมูล 16/32-bit สำหรับคำนวณ ค่านับ และงานทั่วไป",
    hw_r6_desc: "รีจิสเตอร์ข้อมูลชนิดจำค่าเมื่อไฟดับ (Retentive Flash Memory)",
    hw_r7_desc: "อนาล็อกอินพุต 12-bit (0..4095) และเซนเซอร์วัดอุณหภูมิภายในชิป (%IW3)",
    hw_r8_desc: "พอร์ต RS-485 (เลือกคู่พิน GP8/GP9 หรือ GP4/GP5, ขา DE/RE และ Baud 9600..115200 ผ่าน UI ได้ทันที)",
    hw_r_uart0_desc: "พอร์ต Serial อเนกประสงค์ (115200 8N1) สำหรับบล็อกคำสั่งรับ-ส่งข้อมูล UART_SEND / UART_RECV",
    hw_r9_desc: "สัญญาณนาฬิกาพัลส์ 10ms..60s, First Scan (%SM2) และค่า Scan Time / อุณหภูมิ CPU (%SD6)",

    iocfg_win_title: "🔧 Configure RP2040 Hardware I/O Pins — ตั้งค่าพิน GPIO & RS-485 ผ่านโปรแกรมโดยไม่ต้อง Build Firmware ใหม่!",
    iocfg_heading: "🔧 RP2040 Runtime GPIO & RS-485 Pin Mapping",
    iocfg_btn_read: "📥 Read from PLC (0x0B)",
    iocfg_btn_apply: "⚡ Apply & Save to PLC Flash (0x0C)",
    iocfg_btn_reset: "↺ Reset Default",
    iocfg_rs485_title: "📡 RS-485 (Modbus RTU):",
    iocfg_uart1_lbl: "UART1 TX/RX:",
    iocfg_dere_lbl: "DE/RE Pin:",
    iocfg_baud_lbl: "Baud Rate:",
    iocfg_led_lbl: "Status LED Pin:",
    iocfg_pullup_lbl: "Input Pull-Up",
    iocfg_activelow_lbl: "Input Active-Low (GND=ON)",
    iocfg_hb_lbl: "RUN LED 1Hz Heartbeat",
    iocfg_toast_ready: "✔ พร้อมปรับตั้งค่าพินแบบ Runtime ผ่าน USB-CDC (CMD 0x0B / 0x0C)",

    wire_1_tag: "01 / DIGITAL INPUT WIRING",
    wire_1_title: "การต่อสวิตช์ & เซนเซอร์อินพุต (%IX0.0..%IX0.7)",
    wire_1_desc: "ค่าเริ่มต้น X0..X3 = GP10..GP13 (Internal Pull-Up, Active-Low ทริกเกอร์ด้วย GND หรือสลับเป็น Active-High 3.3V ได้ในหน้า Configure I/O Pins) สำหรับงาน 24VDC แนะนำให้ต่อผ่าน Optocoupler (PC817)",
    wire_2_tag: "02 / DIGITAL OUTPUT WIRING",
    wire_2_title: "การต่อรีเลย์ & โซลินอยด์เอาต์พุต (%QX0.0..%QX0.7)",
    wire_2_desc: "ค่าเริ่มต้น Y0..Y5 = GP15, GP14, GP7, GP6, GP2, GP3 (สามารถเปลี่ยนพินได้อิสระในหน้า Configure I/O Pins) จ่ายลอจิก 3.3V แนะนำให้ต่อเข้าโมดูล Relay Opto-Isolated หรือ ULN2803",
    wire_3_tag: "03 / RS-485 MODBUS RTU",
    wire_3_title: "การต่อโมดูล RS-485 (MB_MASTER — UART1)",
    wire_3_desc: "เชื่อมต่อโมดูล MAX485 / SP3485 (3.3V) เข้ากับ UART1: เลือกคู่พิน GP8(TX)/GP9(RX) หรือ GP4(TX)/GP5(RX), กำหนดขา DE/RE (Auto หรือ GP6/GP7/GP16) และ Baud Rate 9600..115200 ได้จากหน้า UI",

    ex_tag: "BUNDLED IEC 61131-3 EXAMPLES",
    ex_title: "ตัวอย่างวงจรมาตรฐานพร้อมใช้งานกว่า 14 โปรเจกต์",
    ex_subtitle: "คลิกเลือกดูโค้ด Instruction List (.il) ตัวอย่างที่มาพร้อมกับโปรแกรม สามารถคัดลอกหรือเปิดจากโฟลเดอร์ example/ ในโปรแกรมเพื่อจำลองการทำงานได้ทันที",
    ex_tab_0: "01 • Motor Start/Stop",
    ex_tab_1: "02 • Star-Delta Timer",
    ex_tab_2: "09 • Modbus RTU Master",
    ex_tab_3: "12 • PID & Analog Scale",
    btn_copy_il: "📋 คัดลอกโค้ด .il",

    rm_tag: "DEVELOPMENT ROADMAP",
    rm_title: "แผนการพัฒนาสู่การปล่อยเวอร์ชันทางการ",
    rm_subtitle: "ติดตามความคืบหน้าของการพัฒนา LaduinoPLC ตั้งแต่รุ่นทดสอบ (Pre-Release) ไปจนถึงการเปิดตัวเวอร์ชันสมบูรณ์",
    rm_p1_badge: "✔ เสร็จสมบูรณ์ (Completed)",
    rm_p1_title: "Core IEC 61131-3 IDE & 32-bit Micro-VM",
    rm_p1_l1: "ตัววาดวงจร Ladder Diagram พร้อมแปลงโค้ด .il แบบสองทิศทาง",
    rm_p1_l2: "ระบบจำลองการทำงานแบบเรียลไทม์ (Offline Simulator) & Force I/O",
    rm_p1_l3: "เฟิร์มแวร์ RP2040 Embassy พร้อม RETAIN Flash & CRC16",
    rm_p2_badge: "🚧 กำลังดำเนินการ (Pre-Release)",
    rm_p2_title: "ปรับปรุงและทดสอบก่อนปล่อยอย่างเป็นทางการ",
    rm_p2_l1: "ทดสอบความเสถียรของบล็อก PID, SCALE, Drum Sequencer และ Modbus RTU",
    rm_p2_l2: "ระบบแฟลชเฟิร์มแวร์ .uf2 ผ่าน USB BOOTSEL ในคลิกเดียว",
    rm_p2_l3: "ตรวจสอบความเข้ากันได้บน Windows 10/11 และ Linux หลากหลายดิสโทร",
    rm_p3_badge: "🎯 เป้าหมายถัดไป (Official Release)",
    rm_p3_title: "Official Public Release & Next-Gen MCU",
    rm_p3_l1: "เปิดตัวเวอร์ชันทางการพร้อมระบบอัปเดตออนไลน์อัตโนมัติเต็มรูปแบบ",
    rm_p3_l2: "รองรับชิปใหม่ Raspberry Pi Pico 2 (RP2350 Cortex-M33)",
    rm_p3_l3: "ขยายการรองรับไมโครคอนโทรลเลอร์ตระกูลอื่น เช่น ESP32 (ESP32 / ESP32-S3 / ESP32-C3) และ STM32",

    sha_title: "🔐 SHA-256 Checksums (ตรวจสอบความถูกต้องของไฟล์)",
    sha_desc: "คุณสามารถตรวจสอบค่า SHA-256 ของไฟล์ที่ดาวน์โหลดจากโฟลเดอร์ releases/ ได้ดังนี้:",
    faq_title: "💡 คำแนะนำการเปิดใช้งานครั้งแรก (First-Run Tips)",
    faq_win_q: "🪟 สำหรับ Windows (SmartScreen):",
    faq_win_a: "หาก Windows แจ้งเตือนไฟล์ใหม่ ให้คลิกปุ่ม More info แล้วกด Run anyway เพื่อเปิดโปรแกรมได้ทันที",
    faq_lin_q: "🐧 สำหรับ Linux (สิทธิ์การรัน & พอร์ต USB Serial):",
    faq_lin_a: "เปิดสิทธิ์รันไฟล์และสิทธิ์เข้าถึงพอร์ต /dev/ttyACM0 ด้วยคำสั่ง:",

    dl_tag: "DOWNLOAD RELEASES",
    dl_title: "ดาวน์โหลด LaduinoPLC & เฟิร์มแวร์",
    dl_subtitle: "ฟรีและโอเพนซอร์ส — ดาวน์โหลดเวอร์ชันล่าสุดได้โดยตรงจาก GitHub Releases",
    an_views_lbl: "👁️ ยอดเข้าชมเว็บไซต์ (Page Views)",
    an_dl_lbl: "⬇️ ยอดดาวน์โหลดรวม (Total Downloads)",
    an_stars_lbl: "⭐ ดาวบน GitHub (GitHub Stars)",
    an_env_lbl: "💻 ระบบที่ตรวจพบ (Detected OS)",
    footer_views_lbl: "ผู้เข้าชม:",
    dl_win_sub: "สำหรับ Windows 10 / 11 (64-bit)",
    dl_win_l1: "ไฟล์โปรแกรมพร้อมรัน (laduino-plc.exe)",
    dl_win_l2: "รวมไฟล์เฟิร์มแวร์ RP2040 (.uf2) ในตัว",
    dl_win_l3: "รวมตัวอย่างวงจร Ladder มาตรฐานครบชุด",
    dl_win_btn: "⬇ ดาวน์โหลดสำหรับ Windows",

    dl_lin_sub: "สำหรับ Ubuntu / Debian / Arch / Fedora (x86_64)",
    dl_lin_l1: "ไบนารีเนทีฟความเร็วสูง (wgpu / X11)",
    dl_lin_l2: "รองรับพอร์ต USB-CDC (/dev/ttyACM*) อัตโนมัติ",
    dl_lin_l3: "รวมไฟล์เฟิร์มแวร์ .uf2 และตัวอย่างครบชุด",
    dl_lin_btn: "⬇ ดาวน์โหลดสำหรับ Linux",

    dl_fw_sub: "32-bit Micro-VM Runtime สำหรับ Raspberry Pi Pico",
    dl_fw_l1: "แฟลชง่ายผ่าน USB BOOTSEL (ลากวางไฟล์ .uf2)",
    dl_fw_l2: "รองรับ Online Monitoring, Force I/O, RETAIN Flash",
    dl_fw_l3: "รองรับ PID, SCALE, LIMIT, UART, Modbus RTU",
    dl_fw_btn: "⚡ ดาวน์โหลดเฟิร์มแวร์ (.uf2)",

    don_tag: "SUPPORT & DONATE",
    don_title: "ร่วมสนับสนุนการพัฒนา LaduinoPLC",
    don_subtitle: "หากโปรแกรม LaduinoPLC ช่วยให้งานของคุณง่ายขึ้น หรือเป็นประโยชน์ต่อการเรียนรู้ คุณสามารถร่วมบริจาคสนับสนุนทีมผู้พัฒนา (pksofttech.org) เพื่อเป็นกำลังใจในการพัฒนาฟีเจอร์ใหม่ๆ ต่อไปได้ครับ",
    qr_scan_label: "สแกน QR Code ด้วยแอปธนาคาร (PromptPay / พร้อมเพย์)",
    qr_zoom_hint: "🔍 คลิกที่รูป QR เพื่อขยายใหญ่",
    btn_copy_id: "คัดลอก",
    btn_copied: "✔ คัดลอกแล้ว!",
    don_pp_title: "🇹🇭 โอนผ่านพร้อมเพย์ / ธนาคารในไทย (PromptPay)",
    don_pp_desc: "สแกน QR Code ด้านซ้ายด้วยแอปพลิเคชันธนาคารใดก็ได้ หรือคัดลอกหมายเลขพร้อมเพย์เพื่อโอนสนับสนุนได้โดยตรง ไม่มีค่าธรรมเนียม",
    don_gh_title: "💜 GitHub Sponsors & กดดาวโปรเจกต์",
    don_gh_desc: "ร่วมเป็นผู้สนับสนุนรายเดือนหรือรายครั้งผ่าน GitHub Sponsors หรือช่วยกด ⭐ Star ให้โปรเจกต์บน GitHub",
    don_gh_btn: "💜 GitHub Sponsors",
    don_star_btn: "⭐ Star on GitHub",
    don_bmc_title: "☕ Buy Me a Coffee",
    don_bmc_desc: "เลี้ยงกาแฟทีมผู้พัฒนาเพื่อเป็นกำลังใจในการพัฒนาฟีเจอร์ใหม่ๆ ผ่านทาง Buy Me a Coffee ได้อย่างสะดวกและปลอดภัย",
    don_bmc_btn: "☕ Buy Me a Coffee",

    rep_tag: "BUG REPORT & FEEDBACK",
    rep_title: "แจ้งปัญหาการใช้งาน & รายงานข้อผิดพลาดของระบบ",
    rep_subtitle: "เนื่องจากปัจจุบันอยู่ในช่วงปรับปรุงก่อนปล่อยอย่างเป็นทางการ (Pre-Release) หากคุณพบข้อผิดพลาด (Bug), ปัญหาการจำลองวงจร หรือมีข้อเสนอแนะ สามารถส่งรายงานถึงทีมพัฒนาได้โดยตรงครับ",
    rep_direct_tag: "DIRECT DEVELOPER CONTACT",
    rep_card_title: "ส่งรายงานผ่านอีเมลถึงทีมพัฒนา",
    rep_card_desc: "คลิกปุ่มด้านล่างเพื่อเปิดโปรแกรมส่งอีเมลพร้อมแบบฟอร์มแจ้งปัญหาอัตโนมัติ หรือคัดลอกที่อยู่อีเมลเพื่อส่งไฟล์วงจร (.il) และภาพหน้าจอประกอบ",
    btn_copy_email: "คัดลอกอีเมล",
    rep_btn_email: "✉️ ส่งอีเมลแจ้งปัญหา (pksofttech@gmail.com)",
    rep_btn_gh: "🐞 แจ้งผ่าน GitHub Issues",
    rep_tips_title: "📋 ข้อมูลที่แนะนำให้แนบมาพร้อมรายงาน",
    rep_tip_1: "ระบบปฏิบัติการที่ใช้งาน (เช่น Windows 10/11 หรือ Ubuntu/Arch Linux)",
    rep_tip_2: "ลำดับขั้นตอนที่ทำให้เกิดปัญหา หรือแนบไฟล์โปรเจกต์วงจร (.il) ที่พบอาการ",
    rep_tip_3: "ภาพถ่ายหน้าจอ (Screenshot) หรือข้อความแจ้งเตือนที่แสดงบนโปรแกรม",
    rep_tip_4: "ข้อมูลบอร์ดไมโครคอนโทรลเลอร์ (กรณีพบปัญหาการเชื่อมต่อพอร์ต USB หรือการแฟลช .uf2)",

    footer_copy: "พัฒนาโดย pksofttech.org • ซอฟต์แวร์ออกแบบและจำลองวงจร PLC มาตรฐาน IEC 61131-3",
    modal_close: "ปิดหน้าต่าง (Close)"
  },

  en: {
    nav_features: "Features",
    nav_getting_started: "Getting Started",
    nav_hardware: "Hardware & Pinout",
    nav_examples: "Examples",
    nav_roadmap: "Roadmap",
    nav_download: "Download",
    nav_report: "Report Issue",
    nav_donate: "☕ Donate / Support",

    hero_badge: "IEC 61131-3 Ladder IDE & RP2040 Micro-VM",
    status_prerelease_short: "Status: Refining before official release",
    status_prerelease_tag: "PRE-RELEASE",
    status_prerelease_desc: "Under final refinement before official release — Pre-release build for stability & feature verification.",
    hero_title_1: "Industrial PLC Ladder IDE",
    hero_title_2: "Simulate & Flash to Hardware in One Click",
    hero_desc:
      "LaduinoPLC by pksofttech.org — A fast, modern IEC 61131-3 Ladder Diagram IDE with a built-in Real-Time Simulator, bidirectional Instruction List (.il) code generator, and a 32-bit Micro-VM firmware runtime for Raspberry Pi Pico (RP2040).",
    btn_download_now: "⬇ Free Download (GitHub Releases)",
    btn_quickstart: "🚀 Getting Started Guide",
    btn_support_dev: "💖 Support the Developer",

    stat_1_val: "100% IEC",
    stat_1_lbl: "IEC 61131-3 IL & Ladder Standard",
    stat_2_val: "32-bit VM",
    stat_2_lbl: "RP2040 Embassy Async Runtime",
    stat_3_val: "Real-Time",
    stat_3_lbl: "Online Monitoring & Force I/O",

    sim_window_title: "LaduinoPLC — Live Interactive Preview (Click buttons below!)",
    sim_comment: "// Network 01: Motor Start / Stop Control with Self-Holding Seal-in Contact",
    sim_hint: "💡 Click %IX0.0 (START) or %IX0.1 (STOP) below to test live power flow in your browser:",

    feat_tag: "KEY CAPABILITIES",
    feat_title: "Everything You Need for Modern Micro-PLC Automation",
    feat_subtitle: "Built for engineering education, rapid industrial prototyping, and embedded microcontroller PLC control.",

    f1_title: "IEC 61131-3 Ladder & IL Editor",
    f1_desc: "Fast CAD-grade ladder editor supporting Contacts (NO/NC/P/N), Coils (Normal/Negated/Set/Reset), Parallel Branches, multi-line Network Comments, and lossless .il round-trip.",
    f2_title: "Industrial Function Blocks",
    f2_desc: "Rich built-in function blocks: Timers (TON, TOF, TP), Counters (CTU, CTD, CTUD), PID Controller, Linear SCALE, LIMIT, MATH, MOVE, BITWISE, Shift Registers, ADC, and UART.",
    f3_title: "Real-Time Simulator & Online Monitor",
    f3_desc: "Test logic immediately on your PC or connect to an RP2040 board over USB-CDC for live power-flow visualization, Watch Table inspection, and real-time I/O forcing.",
    f4_title: "RP2040 32-bit Micro-VM & RETAIN Flash",
    f4_desc: "Compiles ladder networks into compact 32-bit bytecode with CRC16 verification, burning programs and retentive variables (M64..M95, D96..D127) directly to RP2040 Flash.",
    f5_title: "Modbus RTU Master (RS-485)",
    f5_desc: "Poll industrial temperature sensors, power meters, and VFD inverters over RS-485 using the MB_MASTER block (Function Codes 03, 04, 06) with automatic timeout handling.",
    f6_title: "1-Click USB BOOTSEL Flasher & Auto-Update",
    f6_desc: "Flash pre-bundled .uf2 firmware images straight to a Raspberry Pi Pico in BOOTSEL mode from inside the IDE, plus built-in online software update checks.",

    start_tag: "GETTING STARTED",
    start_title: "Get Started with LaduinoPLC in 4 Simple Steps",
    start_subtitle: "Launching LaduinoPLC opens a clean New Ladder workspace so you can start building immediately.",

    s1_title: "Download & Launch",
    s1_desc: "Download the latest Windows or Linux archive from GitHub Releases, extract it, and run laduino-plc — no external runtime dependencies required.",
    s1_box: "Starts directly with a clean New Ladder (1 empty network), or browse 18+ ready-to-run circuits under the Examples menu.",

    s2_title: "Place Elements & Assign IEC Addresses",
    s2_desc: "Pick Contacts, Coils, or Function Blocks from the left panel and drop them onto a rung. Double-click any element to configure its IEC address (%IX0.0, %QX0.0, T0) and Symbol.",
    s2_box: "Shortcuts: P = Parallel Branch | S = Series Element | Enter = Edit Selected | Ctrl+F = Find & Replace",

    s3_title: "Test with the Built-in Simulator",
    s3_desc: "Select Simulator mode and press ▶ Run (F5). Click any input contact directly on the canvas to toggle ON/OFF states and monitor values in the right-hand Watch Table.",
    s3_box: "Sim Shortcuts: F5 = Run / Stop | Shift+F5 = Reset | F6 = Single Scan Step",

    s4_title: "Flash Firmware & Upload to RP2040",
    s4_desc: "Open ⚡ Update to PLC → Firmware Flasher to flash the .uf2 runtime to your Pico in BOOTSEL mode once, then click ⬆ Upload to PLC to burn and run your ladder program.",
    s4_box: "Press F7 (Online Monitor) to view live hardware power conduction with automatic CRC16 code verification.",

    hw_tag: "HARDWARE & MEMORY MAP",
    hw_title: "Standard Pinout & Runtime Configurable I/O (RP2040 Micro-VM)",
    hw_subtitle: "Default hardware pin assignments for Raspberry Pi Pico (RP2040) — all GPIO & RS-485 pins are runtime-configurable from the IDE without rebuilding firmware.",
    th_region: "Memory Region",
    th_iec: "IEC 61131-3 Address",
    th_alias: "PLC Alias",
    th_pin: "RP2040 Hardware Pin (UI Configurable)",
    th_desc: "Description",
    hw_r1_desc: "8 Digital Inputs (Runtime-configurable GPIO, Pull-Up/Down & Active-Low/High via IDE)",
    hw_r2_desc: "8 Digital Outputs (Default Y0..Y5 = GP15,14,7,6,2,3; remappable & saved to Flash 0x1FD000)",
    hw_r_led_desc: "Onboard PLC Status LED (Selectable 1Hz Heartbeat or Solid ON, runtime-remappable pin)",
    hw_r3_desc: "General-purpose internal bit relays (cleared on PLC reset)",
    hw_r4_desc: "Retentive bit relays automatically backed up to RP2040 Flash",
    hw_r5_desc: "General-purpose signed 32-bit word registers for math & data",
    hw_r6_desc: "Retentive word registers backed up to non-volatile RP2040 Flash",
    hw_r7_desc: "12-bit Analog Inputs (0..4095) & onboard die temperature sensor (%IW3)",
    hw_r8_desc: "RS-485 Half-Duplex UART1 (Selectable GP8/GP9 or GP4/GP5, DE/RE pin & 9600..115200 bps in UI)",
    hw_r_uart0_desc: "User Serial UART0 (GP0=TX, GP1=RX @ 115200 8N1) for UART_SEND / UART_RECV function blocks",
    hw_r9_desc: "System clock pulses (10ms..60s), First Scan (%SM2), Scan Time & CPU Temp (%SD6)",

    iocfg_win_title: "🔧 Configure RP2040 Hardware I/O Pins — Remap GPIO & RS-485 at Runtime without Firmware Rebuild!",
    iocfg_heading: "🔧 RP2040 Runtime GPIO & RS-485 Pin Mapping",
    iocfg_btn_read: "📥 Read from PLC (0x0B)",
    iocfg_btn_apply: "⚡ Apply & Save to PLC Flash (0x0C)",
    iocfg_btn_reset: "↺ Reset Default",
    iocfg_rs485_title: "📡 RS-485 (Modbus RTU):",
    iocfg_uart1_lbl: "UART1 TX/RX:",
    iocfg_dere_lbl: "DE/RE Pin:",
    iocfg_baud_lbl: "Baud Rate:",
    iocfg_led_lbl: "Status LED Pin:",
    iocfg_pullup_lbl: "Input Pull-Up",
    iocfg_activelow_lbl: "Input Active-Low (GND=ON)",
    iocfg_hb_lbl: "RUN LED 1Hz Heartbeat",
    iocfg_toast_ready: "✔ Ready to configure GPIO & RS-485 at runtime via USB-CDC (CMD 0x0B / 0x0C)",

    wire_1_tag: "01 / DIGITAL INPUT WIRING",
    wire_1_title: "Digital Input Wiring (%IX0.0..%IX0.7)",
    wire_1_desc: "Default X0..X3 = GP10..GP13 (Internal Pull-Up, Active-Low GND trigger, or switch to Active-High 3.3V in Configure I/O Pins). For 24VDC sensors, wire through a PC817 optocoupler.",
    wire_2_tag: "02 / DIGITAL OUTPUT WIRING",
    wire_2_title: "Relay & Solenoid Outputs (%QX0.0..%QX0.7)",
    wire_2_desc: "Default Y0..Y5 = GP15, GP14, GP7, GP6, GP2, GP3 (freely remappable in Configure I/O Pins). Outputs 3.3V logic — connect to opto-isolated relays or a ULN2803 driver.",
    wire_3_tag: "03 / RS-485 MODBUS RTU",
    wire_3_title: "RS-485 Transceiver Wiring (MB_MASTER — UART1)",
    wire_3_desc: "Connect a 3.3V MAX3485 / MAX485 transceiver to UART1: choose GP8(TX)/GP9(RX) or GP4(TX)/GP5(RX), DE/RE pin (Auto or GP6/GP7/GP16), and Baud Rate 9600..115200 in the UI.",

    ex_tag: "BUNDLED IEC 61131-3 EXAMPLES",
    ex_title: "14+ Ready-to-Run Industrial Ladder Examples",
    ex_subtitle: "Click any tab below to inspect the IEC 61131-3 Instruction List (.il) code bundled in the example/ directory.",
    ex_tab_0: "01 • Motor Start/Stop",
    ex_tab_1: "02 • Star-Delta Timer",
    ex_tab_2: "09 • Modbus RTU Master",
    ex_tab_3: "12 • PID & Analog Scale",
    btn_copy_il: "📋 Copy .il Code",

    rm_tag: "DEVELOPMENT ROADMAP",
    rm_title: "Roadmap to Official Release",
    rm_subtitle: "Track LaduinoPLC's progress from the current pre-release refinement phase to the official public launch.",
    rm_p1_badge: "✔ Completed",
    rm_p1_title: "Core IEC 61131-3 IDE & 32-bit Micro-VM",
    rm_p1_l1: "Bidirectional Ladder Diagram & IEC 61131-3 .il code editor",
    rm_p1_l2: "Real-Time Offline Simulator with live conduction & Force I/O",
    rm_p1_l3: "RP2040 Embassy async runtime with RETAIN Flash & CRC16 check",
    rm_p2_badge: "🚧 In Progress (Pre-Release)",
    rm_p2_title: "Refinement & Pre-Release Verification",
    rm_p2_l1: "Hardware-in-the-loop stress testing for PID, SCALE, Drum & Modbus RTU",
    rm_p2_l2: "1-Click USB BOOTSEL (.uf2) firmware flasher integration",
    rm_p2_l3: "Cross-platform verification across Windows 10/11 & Linux distros",
    rm_p3_badge: "🎯 Upcoming (Official Release)",
    rm_p3_title: "Official Public Release & Next-Gen MCU",
    rm_p3_l1: "Official stable release with full online auto-updater support",
    rm_p3_l2: "Expanded support for Raspberry Pi Pico 2 (RP2350 Cortex-M33)",
    rm_p3_l3: "Multi-architecture MCU runtime support for ESP32 (ESP32 / ESP32-S3 / ESP32-C3) & STM32",

    sha_title: "🔐 SHA-256 Checksums (Verify Integrity)",
    sha_desc: "Verify your downloaded release packages using these SHA-256 hashes:",
    faq_title: "💡 First-Run Quick Tips",
    faq_win_q: "🪟 Windows SmartScreen Notice:",
    faq_win_a: "If Windows SmartScreen prompts on first launch, click More info → Run anyway to start LaduinoPLC.",
    faq_lin_q: "🐧 Linux Executable & USB Serial Permissions:",
    faq_lin_a: "Grant executable permission and /dev/ttyACM0 serial access with:",

    dl_tag: "DOWNLOAD RELEASES",
    dl_title: "Download LaduinoPLC & Firmware",
    dl_subtitle: "Free & Open-Source — Get the latest pre-built packages directly from GitHub Releases.",
    an_views_lbl: "👁️ Total Website Visits (Page Views)",
    an_dl_lbl: "⬇️ Total Release Downloads",
    an_stars_lbl: "⭐ GitHub Stars & Forks",
    an_env_lbl: "💻 Detected OS & Browser",
    footer_views_lbl: "Visitors:",
    dl_win_sub: "For Windows 10 / 11 (64-bit)",
    dl_win_l1: "Standalone executable (laduino-plc.exe)",
    dl_win_l2: "Includes bundled RP2040 .uf2 firmware",
    dl_win_l3: "Includes 18+ IEC 61131-3 example programs",
    dl_win_btn: "⬇ Download for Windows",

    dl_lin_sub: "For Ubuntu / Debian / Arch / Fedora (x86_64)",
    dl_lin_l1: "Fast native binary (wgpu / X11)",
    dl_lin_l2: "Automatic USB-CDC (/dev/ttyACM*) detection",
    dl_lin_l3: "Includes bundled .uf2 firmware & examples",
    dl_lin_btn: "⬇ Download for Linux",

    dl_fw_sub: "32-bit Micro-VM Runtime for Raspberry Pi Pico",
    dl_fw_l1: "Drag-and-drop USB BOOTSEL (.uf2) flashing",
    dl_fw_l2: "Supports Online Monitor, Force I/O & RETAIN Flash",
    dl_fw_l3: "Supports PID, SCALE, LIMIT, UART & Modbus RTU",
    dl_fw_btn: "⚡ Download Firmware (.uf2)",

    don_tag: "SUPPORT & DONATE",
    don_title: "Support LaduinoPLC Development",
    don_subtitle: "If LaduinoPLC helps your projects, teaching, or automation work, consider supporting pksofttech.org to help fund ongoing development and new hardware features.",
    qr_scan_label: "Scan via Thai Banking App (PromptPay QR)",
    qr_zoom_hint: "🔍 Click QR image to enlarge",
    btn_copy_id: "Copy",
    btn_copied: "✔ Copied!",
    don_pp_title: "🇹🇭 PromptPay / Thai Bank Transfer",
    don_pp_desc: "Scan the PromptPay QR code on the left with any mobile banking app in Thailand, or copy the PromptPay ID below.",
    don_gh_title: "💜 GitHub Sponsors & Star Repository",
    don_gh_desc: "Sponsor ongoing open-source work via GitHub Sponsors or give the repository a ⭐ Star on GitHub.",
    don_gh_btn: "💜 GitHub Sponsors",
    don_star_btn: "⭐ Star on GitHub",
    don_bmc_title: "☕ Buy Me a Coffee",
    don_bmc_desc: "Buy the developer a coffee to support ongoing development via Buy Me a Coffee.",
    don_bmc_btn: "☕ Buy Me a Coffee",

    rep_tag: "BUG REPORT & FEEDBACK",
    rep_title: "Report System Bugs & Send Feedback",
    rep_subtitle: "As LaduinoPLC is currently in Pre-Release refinement, please report any bugs, simulation issues, or feature suggestions directly to the developer.",
    rep_direct_tag: "DIRECT DEVELOPER CONTACT",
    rep_card_title: "Send Bug Report via Email",
    rep_card_desc: "Click below to launch your email client with a pre-filled report template, or copy the email address to attach .il files and screenshots.",
    btn_copy_email: "Copy Email",
    rep_btn_email: "✉️ Email Bug Report (pksofttech@gmail.com)",
    rep_btn_gh: "🐞 Open GitHub Issue",
    rep_tips_title: "📋 Helpful Details to Include in Your Report",
    rep_tip_1: "Operating System version (e.g. Windows 10/11 or Ubuntu/Arch Linux)",
    rep_tip_2: "Steps to reproduce the issue, or attach the .il ladder circuit file",
    rep_tip_3: "Screenshots or any error messages displayed in the IDE",
    rep_tip_4: "MCU board details (if reporting USB-CDC or .uf2 flashing issues)",

    footer_copy: "Developed by pksofttech.org • IEC 61131-3 PLC Ladder Diagram IDE & Micro-VM Runtime",
    modal_close: "Close Window"
  }
};

let currentLang = localStorage.getItem("laduino_lang") || "th";

// Interactive Hero Ladder State
const simState = {
  ix0_0: false, // START_PB (NO)
  ix0_1: false, // STOP_PB (NC -> false = closed/conducting, true = pressed/open)
  qx0_0: false  // MOTOR_RUN (Coil + Seal-in contact)
};

function evaluateSim() {
  const startBranch = simState.ix0_0 || simState.qx0_0;
  const stopClosed = !simState.ix0_1;
  simState.qx0_0 = startBranch && stopClosed;
  renderSimSvg();
}

function renderSimSvg() {
  const hotColor = "#10e182";
  const offColor = "#475569";
  const cyanColor = "#00f0ff";

  const wStart = simState.ix0_0 ? hotColor : cyanColor;
  const wSeal = simState.qx0_0 ? hotColor : cyanColor;
  const branchOut = simState.ix0_0 || simState.qx0_0;
  const wMid = branchOut ? hotColor : offColor;
  const wStop = !simState.ix0_1 ? (branchOut ? hotColor : cyanColor) : "#ef4444";
  const wCoil = simState.qx0_0 ? hotColor : offColor;

  const setStroke = (id, col) => {
    const el = document.getElementById(id);
    if (el) el.setAttribute("stroke", col);
  };

  setStroke("wire-start", simState.ix0_0 ? hotColor : offColor);
  setStroke("contact-start-1", wStart);
  setStroke("contact-start-2", wStart);

  setStroke("wire-seal", simState.qx0_0 ? hotColor : offColor);
  setStroke("contact-seal-1", wSeal);
  setStroke("contact-seal-2", wSeal);

  setStroke("wire-mid", wMid);
  setStroke("contact-stop-1", wStop);
  setStroke("contact-stop-2", wStop);
  setStroke("contact-stop-slash", wStop);

  setStroke("wire-out", wCoil);
  setStroke("coil-left", wCoil);
  setStroke("coil-right", wCoil);

  const btnStart = document.getElementById("btn-sim-start");
  const btnStop = document.getElementById("btn-sim-stop");
  const motorBadge = document.getElementById("sim-motor-badge");

  if (btnStart) {
    btnStart.classList.toggle("active-on", simState.ix0_0);
    btnStart.textContent = `%IX0.0 START_PB: ${simState.ix0_0 ? "ON (1)" : "OFF (0)"}`;
  }
  if (btnStop) {
    btnStop.classList.toggle("active-stop", simState.ix0_1);
    btnStop.textContent = `%IX0.1 STOP_PB (NC): ${simState.ix0_1 ? "PRESSED (1)" : "NORMAL (0)"}`;
  }
  if (motorBadge) {
    motorBadge.classList.toggle("energized", simState.qx0_0);
    motorBadge.textContent = simState.qx0_0
      ? "● %QX0.0 MOTOR_RUN = ON (1)"
      : "○ %QX0.0 MOTOR_RUN = OFF (0)";
  }

  const ilCode = document.getElementById("sim-il-preview");
  if (ilCode) {
    ilCode.textContent =
      `(* Network 01: Motor Start/Stop [Energized=${simState.qx0_0 ? 1 : 0}] *)\n` +
      `    LD   %IX0.0    (* START_PB = ${simState.ix0_0 ? 1 : 0} *)\n` +
      `    OR   %QX0.0    (* MOTOR_RUN Seal-in = ${simState.qx0_0 ? 1 : 0} *)\n` +
      `    ANDN %IX0.1    (* STOP_PB NC = ${!simState.ix0_1 ? 1 : 0} *)\n` +
      `    ST   %QX0.0    (* Output Coil => ${simState.qx0_0 ? 1 : 0} *)`;
  }
}

// Bundled IEC 61131-3 Example Programs Preview
const EXAMPLES_DATA = [
  {
    titleTh: "01_motor_start_stop.il — วงจรควบคุมมอเตอร์ Start / Stop ค้างสถานะ",
    titleEn: "01_motor_start_stop.il — Motor Start / Stop with Self-Holding Seal-in",
    descTh: "พื้นฐานการเขียนวงจร Self-Holding (Latching) ร่วมกับปุ่มหยุดฉุกเฉิน NC และไฟแสดงสถานะการทำงาน",
    descEn: "Standard industrial 3-wire motor control with parallel seal-in contact and NC overload/stop interlock.",
    code: `(* ======================================================== *)
(* Example 01: Motor Start / Stop with Self-Holding         *)
(* ======================================================== *)
(* Network 01: Main Contactor Latch                         *)
    LD   %IX0.0        (* START_PB (NO) *)
    OR   %QX0.0        (* MOTOR_RUN Seal-in *)
    ANDN %IX0.1        (* STOP_PB (NC) *)
    ANDN %IX0.2        (* OVERLOAD_TRIP (NC) *)
    ST   %QX0.0        (* MOTOR_RUN Output *)

(* Network 02: Run & Fault Pilot Lamps                      *)
    LD   %QX0.0
    ST   %QX0.1        (* GREEN_RUN_LAMP *)
    LD   %IX0.2
    ST   %QX0.2        (* RED_FAULT_LAMP *)`
  },
  {
    titleTh: "02_star_delta_timer.il — วงจรกลับเฟสสตาร์ทมอเตอร์ Star-Delta อัตโนมัติ",
    titleEn: "02_star_delta_timer.il — Automatic Star-Delta Motor Starter with TON Timer",
    descTh: "ใช้บล็อกตั้งเวลา TON (T0) สลับคอนแทคเตอร์จาก Star (%QX0.1) ไปเป็น Delta (%QX0.2) พร้อม Interlock ป้องกันการชนกัน",
    descEn: "Uses an on-delay timer TON (T0 = 5.0s) to transition from Star (%QX0.1) to Delta (%QX0.2) with electrical interlock.",
    code: `(* ======================================================== *)
(* Example 02: Automatic Star-Delta Starter                 *)
(* ======================================================== *)
(* Network 01: Main Contactor KM1 + Transition Timer T0     *)
    LD   %IX0.0        (* START_PB *)
    OR   %QX0.0        (* KM1_MAIN *)
    ANDN %IX0.1        (* STOP_PB *)
    ST   %QX0.0        (* KM1_MAIN *)
    TON  T0, 50        (* 5.0s Star-to-Delta Delay *)

(* Network 02: Star Contactor KM2 (Active before T0 done)   *)
    LD   %QX0.0
    ANDN T0
    ANDN %QX0.2        (* Interlock KM3_DELTA *)
    ST   %QX0.1        (* KM2_STAR *)

(* Network 03: Delta Contactor KM3 (Active after T0 done)   *)
    LD   %QX0.0
    AND  T0
    ANDN %QX0.1        (* Interlock KM2_STAR *)
    ST   %QX0.2        (* KM3_DELTA *)`
  },
  {
    titleTh: "09_modbus_rtu_master.il — อ่านค่าเซนเซอร์และสั่งงานอินเวอร์เตอร์ผ่าน RS-485",
    titleEn: "09_modbus_rtu_master.il — RS-485 Modbus RTU Master Polling & VFD Control",
    descTh: "ใช้สัญญาณพัลส์ 100ms (%SM4) สั่งบล็อก MB_MASTER อ่านค่า Holding Register (FC03) จาก Slave ID 1 มาเก็บใน %MW10",
    descEn: "Uses 100ms system clock (%SM4) to trigger MB_MASTER (FC03 Read Holding Registers) from Slave ID 1 into %MW10.",
    code: `(* ======================================================== *)
(* Example 09: Modbus RTU Master (RS-485 UART1)             *)
(* ======================================================== *)
(* Network 01: Poll Temperature Sensor (Slave=1, FC=03)     *)
    LD   %SM4          (* 100ms System Clock Pulse *)
    MB_MASTER MB0, 1, 3, 0, 2, %MW10

(* Network 02: High Temperature Alarm (> 85.0 C -> 850)     *)
    LD   %SM0          (* Always ON *)
    GT   %MW10, 850
    ST   %QX0.3        (* HIGH_TEMP_ALARM_RELAY *)`
  },
  {
    titleTh: "12_pid_scale_process.il — สเกลค่าอนาล็อก ADC 12-bit และควบคุมด้วย PID",
    titleEn: "12_pid_scale_process.il — 12-bit ADC Linear Scaling & Closed-Loop PID Control",
    descTh: "แปลงค่าอนาล็อก %IW0 (0..4095) เป็นหน่วยวิศวกรรม (0..1000) ด้วยบล็อก SCALE แล้วส่งเข้าบล็อก PID เพื่อคุมเอาต์พุต",
    descEn: "Scales 12-bit ADC input %IW0 (0..4095) to engineering units (0..1000) and computes closed-loop PID control output.",
    code: `(* ======================================================== *)
(* Example 12: Analog Scaling & PID Process Control         *)
(* ======================================================== *)
(* Network 01: Scale ADC0 (%IW0: 0..4095 -> %MW0: 0..1000)  *)
    LD   %SM0
    SCALE %IW0, 0, 4095, 0, 1000, %MW0

(* Network 02: Closed-Loop PID (SP=%MW2, PV=%MW0 -> %MW4)   *)
    LD   %IX0.0        (* AUTO_MODE_ENABLE *)
    PID  PID0, %MW2, %MW0, 120, 15, 5, %MW4`
  }
];

let currentExampleIdx = 0;

function renderExample(idx) {
  currentExampleIdx = idx;
  const item = EXAMPLES_DATA[idx] || EXAMPLES_DATA[0];
  const titleEl = document.getElementById("example-title");
  const descEl = document.getElementById("example-desc");
  const codeEl = document.getElementById("example-code-content");

  if (titleEl) titleEl.textContent = currentLang === "th" ? item.titleTh : item.titleEn;
  if (descEl) descEl.textContent = currentLang === "th" ? item.descTh : item.descEn;
  if (codeEl) codeEl.textContent = item.code;

  document.querySelectorAll(".example-tab").forEach((btn, i) => {
    btn.classList.toggle("active", i === idx);
  });
}

function setLanguage(lang) {
  currentLang = lang === "en" ? "en" : "th";
  localStorage.setItem("laduino_lang", currentLang);
  document.documentElement.lang = currentLang;

  const dict = I18N[currentLang];
  document.querySelectorAll("[data-i18n]").forEach((el) => {
    const key = el.getAttribute("data-i18n");
    if (dict[key]) {
      el.textContent = dict[key];
    }
  });

  document.getElementById("lang-th")?.classList.toggle("active", currentLang === "th");
  document.getElementById("lang-en")?.classList.toggle("active", currentLang === "en");

  const acctNameEl = document.getElementById("promptpay-acct-name");
  if (acctNameEl) {
    acctNameEl.textContent =
      currentLang === "th" ? SITE_CONFIG.promptPayNameTh : SITE_CONFIG.promptPayNameEn;
  }

  renderExample(currentExampleIdx);
}

function applySiteConfig() {
  const bindHref = (id, url) => {
    const el = document.getElementById(id);
    if (el) el.setAttribute("href", url);
  };

  bindHref("hero-dl-btn", SITE_CONFIG.releasesUrl);
  bindHref("dl-win-link", SITE_CONFIG.windowsDownloadUrl);
  bindHref("dl-lin-link", SITE_CONFIG.linuxDownloadUrl);
  bindHref("dl-fw-link", SITE_CONFIG.firmwareUf2Url);
  bindHref("don-gh-link", SITE_CONFIG.githubSponsorsUrl);
  bindHref("don-star-link", SITE_CONFIG.githubRepoUrl);
  bindHref("don-bmc-link", SITE_CONFIG.buyMeACoffeeUrl);
  bindHref("footer-gh-link", SITE_CONFIG.githubRepoUrl);

  const verBadge = document.getElementById("site-version-badge");
  if (verBadge) verBadge.textContent = SITE_CONFIG.version;

  const ppIdEl = document.getElementById("promptpay-id-text");
  if (ppIdEl) ppIdEl.textContent = SITE_CONFIG.promptPayId;

  const qrImg = document.getElementById("promptpay-qr-img");
  const qrModalImg = document.getElementById("promptpay-qr-modal-img");
  if (qrImg) qrImg.setAttribute("src", SITE_CONFIG.promptPayQrImage);
  if (qrModalImg) qrModalImg.setAttribute("src", SITE_CONFIG.promptPayQrImage);
}

document.addEventListener("DOMContentLoaded", () => {
  applySiteConfig();
  setLanguage(currentLang);
  evaluateSim();

  document.getElementById("lang-th")?.addEventListener("click", () => setLanguage("th"));
  document.getElementById("lang-en")?.addEventListener("click", () => setLanguage("en"));

  document.getElementById("btn-sim-start")?.addEventListener("click", () => {
    simState.ix0_0 = !simState.ix0_0;
    evaluateSim();
  });

  document.getElementById("btn-sim-stop")?.addEventListener("click", () => {
    simState.ix0_1 = !simState.ix0_1;
    evaluateSim();
  });

  // Copy PromptPay ID
  const copyBtn = document.getElementById("copy-promptpay-btn");
  if (copyBtn) {
    copyBtn.addEventListener("click", () => {
      navigator.clipboard?.writeText(SITE_CONFIG.promptPayId);
      const dict = I18N[currentLang];
      copyBtn.textContent = dict.btn_copied;
      setTimeout(() => {
        copyBtn.textContent = I18N[currentLang].btn_copy_id;
      }, 1800);
    });
  }

  // Copy Report Email Address
  const copyEmailBtn = document.getElementById("copy-email-btn");
  if (copyEmailBtn) {
    copyEmailBtn.addEventListener("click", () => {
      navigator.clipboard?.writeText(SITE_CONFIG.reportEmail);
      copyEmailBtn.textContent = I18N[currentLang].btn_copied;
      setTimeout(() => {
        copyEmailBtn.textContent = I18N[currentLang].btn_copy_email;
      }, 1800);
    });
  }

  // QR Modal Zoom
  const qrModal = document.getElementById("qr-modal");
  document.getElementById("promptpay-qr-img")?.addEventListener("click", () => {
    qrModal?.classList.add("open");
  });
  document.getElementById("close-qr-modal")?.addEventListener("click", () => {
    qrModal?.classList.remove("open");
  });
  qrModal?.addEventListener("click", (e) => {
    if (e.target === qrModal) qrModal.classList.remove("open");
  });

  // Example Tabs & Copy IL Code
  renderExample(0);
  document.querySelectorAll(".example-tab").forEach((btn) => {
    btn.addEventListener("click", () => {
      const idx = parseInt(btn.getAttribute("data-example") || "0", 10);
      renderExample(idx);
    });
  });

  const copyExBtn = document.getElementById("copy-example-btn");
  if (copyExBtn) {
    copyExBtn.addEventListener("click", () => {
      const item = EXAMPLES_DATA[currentExampleIdx] || EXAMPLES_DATA[0];
      navigator.clipboard?.writeText(item.code);
      copyExBtn.textContent = I18N[currentLang].btn_copied;
      setTimeout(() => {
        copyExBtn.textContent = I18N[currentLang].btn_copy_il;
      }, 1800);
    });
  }

  // Mobile Menu Drawer Toggle
  const mobileMenuBtn = document.getElementById("mobile-menu-btn");
  const navMenu = document.getElementById("nav-links-menu");
  if (mobileMenuBtn && navMenu) {
    const icon = mobileMenuBtn.querySelector(".mobile-menu-icon");
    const closeMobileMenu = () => {
      navMenu.classList.remove("mobile-open");
      mobileMenuBtn.setAttribute("aria-expanded", "false");
      if (icon) icon.textContent = "+";
    };

    mobileMenuBtn.addEventListener("click", (e) => {
      e.stopPropagation();
      const isOpen = navMenu.classList.toggle("mobile-open");
      mobileMenuBtn.setAttribute("aria-expanded", isOpen ? "true" : "false");
      if (icon) icon.textContent = isOpen ? "×" : "+";
    });

    navMenu.querySelectorAll("a").forEach((link) => {
      link.addEventListener("click", () => closeMobileMenu());
    });

    document.addEventListener("click", (e) => {
      if (!navMenu.contains(e.target) && !mobileMenuBtn.contains(e.target)) {
        closeMobileMenu();
      }
    });

    document.addEventListener("keydown", (e) => {
      if (e.key === "Escape") {
        closeMobileMenu();
      }
    });
  }

  // Initialize Interactive Configure RP2040 Hardware I/O Pins Showcase
  initIoConfigShowcase();

  // Initialize Visitor & Download Analytics
  initVisitorAnalytics();
});

/**
 * Interactive "Configure RP2040 Hardware I/O Pins" Showcase in #hardware section
 */
function initIoConfigShowcase() {
  const tbody = document.getElementById("iocfg-pin-rows");
  if (!tbody) return;

  const DEFAULT_IN = [10, 11, 12, 13, 255, 255, 255, 255];
  const DEFAULT_OUT = [15, 14, 7, 6, 2, 3, 255, 255];
  const ALL_PINS = [255, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 29];

  const state = {
    inPins: [...DEFAULT_IN],
    outPins: [...DEFAULT_OUT],
    rs485AltGp4Gp5: false,
    rs485DeRe: 255,
    rs485Baud: 9600,
    ledPin: 25
  };

  const pairEl = document.getElementById("iocfg-rs485-pair");
  const dereEl = document.getElementById("iocfg-rs485-dere");
  const baudEl = document.getElementById("iocfg-rs485-baud");
  const ledEl = document.getElementById("iocfg-led-pin");
  const summaryEl = document.getElementById("iocfg-reserved-summary");
  const toastEl = document.getElementById("iocfg-status-toast");

  const setToast = (msgTh, msgEn, isSuccess = true) => {
    if (!toastEl) return;
    toastEl.textContent = currentLang === "en" ? msgEn : msgTh;
    toastEl.style.color = isSuccess ? "#10e182" : "#38bdf8";
  };

  const renderTable = () => {
    const txPin = state.rs485AltGp4Gp5 ? 4 : 8;
    const rxPin = state.rs485AltGp4Gp5 ? 5 : 9;
    const assignable = ALL_PINS.filter((p) => p === 255 || (p !== txPin && p !== rxPin));

    // Sanitize any conflicting X/Y pins if UART1 pair or DE/RE changed
    for (let i = 0; i < 8; i++) {
      if (state.inPins[i] === txPin || state.inPins[i] === rxPin || (state.rs485DeRe !== 255 && state.inPins[i] === state.rs485DeRe)) {
        state.inPins[i] = 255;
      }
      if (state.outPins[i] === txPin || state.outPins[i] === rxPin || (state.rs485DeRe !== 255 && state.outPins[i] === state.rs485DeRe)) {
        state.outPins[i] = 255;
      }
    }

    if (summaryEl) {
      const dereStr = state.rs485DeRe === 255 ? "Auto-Direction (None)" : `GP${state.rs485DeRe}`;
      summaryEl.textContent =
        `Reserved peripherals: UART0 (GP0=TX, GP1=RX) • Modbus RS-485 UART1 (GP${txPin}=TX, GP${rxPin}=RX, DE/RE=${dereStr}, ${state.rs485Baud} bps) • Analog ADC (GP26..GP28)`;
    }

    tbody.innerHTML = "";
    for (let i = 0; i < 8; i++) {
      const tr = document.createElement("tr");
      const buildOptions = (selectedPin) =>
        assignable
          .map((p) => {
            const label = p === 255 ? "Disabled (-)" : `GP${p}`;
            const sel = p === selectedPin ? " selected" : "";
            return `<option value="${p}"${sel}>${label}</option>`;
          })
          .join("");

      tr.innerHTML = `
        <td class="mono-cell">X${i}</td>
        <td class="mono-cell">%IX0.${i}</td>
        <td><select class="iocfg-select iocfg-in-select" data-idx="${i}">${buildOptions(state.inPins[i])}</select></td>
        <td class="col-sep">│</td>
        <td class="mono-cell">Y${i}</td>
        <td class="mono-cell">%QX0.${i}</td>
        <td><select class="iocfg-select iocfg-out-select" data-idx="${i}">${buildOptions(state.outPins[i])}</select></td>
      `;
      tbody.appendChild(tr);
    }

    tbody.querySelectorAll(".iocfg-in-select").forEach((sel) => {
      sel.addEventListener("change", (e) => {
        const idx = Number(e.target.getAttribute("data-idx"));
        state.inPins[idx] = Number(e.target.value);
        setToast(
          `✔ ตั้งค่า X${idx} (%IX0.${idx}) = ${state.inPins[idx] === 255 ? "Disabled (-)" : "GP" + state.inPins[idx]}`,
          `✔ Mapped X${idx} (%IX0.${idx}) = ${state.inPins[idx] === 255 ? "Disabled (-)" : "GP" + state.inPins[idx]}`
        );
      });
    });

    tbody.querySelectorAll(".iocfg-out-select").forEach((sel) => {
      sel.addEventListener("change", (e) => {
        const idx = Number(e.target.getAttribute("data-idx"));
        state.outPins[idx] = Number(e.target.value);
        setToast(
          `✔ ตั้งค่า Y${idx} (%QX0.${idx}) = ${state.outPins[idx] === 255 ? "Disabled (-)" : "GP" + state.outPins[idx]}`,
          `✔ Mapped Y${idx} (%QX0.${idx}) = ${state.outPins[idx] === 255 ? "Disabled (-)" : "GP" + state.outPins[idx]}`
        );
      });
    });
  };

  if (pairEl) {
    pairEl.addEventListener("change", () => {
      state.rs485AltGp4Gp5 = pairEl.value === "gp4_5";
      renderTable();
      setToast(
        `📡 สลับพิน RS-485 UART1 เป็น ${state.rs485AltGp4Gp5 ? "GP4(TX)/GP5(RX)" : "GP8(TX)/GP9(RX)"} เรียบร้อย`,
        `📡 Switched RS-485 UART1 to ${state.rs485AltGp4Gp5 ? "GP4(TX)/GP5(RX)" : "GP8(TX)/GP9(RX)"}`
      );
    });
  }

  if (dereEl) {
    dereEl.addEventListener("change", () => {
      state.rs485DeRe = Number(dereEl.value);
      renderTable();
      const lbl = state.rs485DeRe === 255 ? "Auto-Direction (None)" : `GP${state.rs485DeRe}`;
      setToast(`📡 ตั้งค่าขา RS-485 DE/RE = ${lbl}`, `📡 Set RS-485 DE/RE Pin = ${lbl}`);
    });
  }

  if (baudEl) {
    baudEl.addEventListener("change", () => {
      state.rs485Baud = Number(baudEl.value);
      renderTable();
      setToast(
        `📡 ตั้งค่าความเร็ว RS-485 Modbus RTU = ${state.rs485Baud} bps`,
        `📡 Set RS-485 Modbus RTU Baud Rate = ${state.rs485Baud} bps`
      );
    });
  }

  if (ledEl) {
    ledEl.addEventListener("change", () => {
      state.ledPin = Number(ledEl.value);
      const lbl = state.ledPin === 255 ? "Disabled (-)" : `GP${state.ledPin}`;
      setToast(`💡 ตั้งค่า Status LED Pin = ${lbl}`, `💡 Set Status LED Pin = ${lbl}`);
    });
  }

  const btnRead = document.getElementById("btn-iocfg-read");
  if (btnRead) {
    btnRead.addEventListener("click", () => {
      setToast(
        "📥 [CMD 0x0B -> 0x8B] อ่านค่า GPIO & RS-485 Pin Mapping (20 Bytes, CRC16 Verified) จากบอร์ด RP2040 สำเร็จ!",
        "📥 [CMD 0x0B -> 0x8B] Loaded 20-byte HardwareIoConfig from RP2040 PLC over USB-CDC!",
        false
      );
    });
  }

  const btnApply = document.getElementById("btn-iocfg-apply");
  if (btnApply) {
    btnApply.addEventListener("click", () => {
      const txPin = state.rs485AltGp4Gp5 ? 4 : 8;
      const rxPin = state.rs485AltGp4Gp5 ? 5 : 9;
      setToast(
        `⚡ [CMD 0x0C -> 0x8C] บันทึกพิน GPIO & RS-485 (GP${txPin}/GP${rxPin} @ ${state.rs485Baud} bps) ลง Flash 0x1FD000 ทันทีโดยไม่ต้อง Build Firmware ใหม่!`,
        `⚡ [CMD 0x0C -> 0x8C] Applied & saved GPIO + RS-485 (GP${txPin}/GP${rxPin} @ ${state.rs485Baud} bps) to RP2040 Flash 0x1FD000!`
      );
    });
  }

  const btnReset = document.getElementById("btn-iocfg-reset");
  if (btnReset) {
    btnReset.addEventListener("click", () => {
      state.inPins = [...DEFAULT_IN];
      state.outPins = [...DEFAULT_OUT];
      state.rs485AltGp4Gp5 = false;
      state.rs485DeRe = 255;
      state.rs485Baud = 9600;
      state.ledPin = 25;
      if (pairEl) pairEl.value = "gp8_9";
      if (dereEl) dereEl.value = "255";
      if (baudEl) baudEl.value = "9600";
      if (ledEl) ledEl.value = "25";
      renderTable();
      setToast(
        "↺ คืนค่าพินเริ่มต้นโรงงาน (X0..X3=GP10..13, Y0..Y5=GP15,14,7,6,2,3, RS-485=GP8/GP9 @ 9600)",
        "↺ Restored factory default RP2040 GPIO & RS-485 pin mapping"
      );
    });
  }

  renderTable();
}

/**
 * ระบบนับสถิติผู้เยี่ยมชมเว็บไซต์และยอดดาวน์โหลด (Visitor & Download Analytics)
 * - นับจำนวนผู้เข้าชมหน้าเว็บผ่าน Abacus Counter API (แยก Session เพื่อไม่ให้นับซ้ำเวลากด Refresh รัวๆ)
 * - ดึงยอดดาวน์โหลดจริงของแต่ละแพลตฟอร์มจาก GitHub Releases API + นับจำนวนคลิกปุ่มดาวน์โหลด
 * - ดึงจำนวน GitHub Stars & Watchers
 * - ตรวจจับ OS / Browser ของผู้เข้าชมเพื่อวิเคราะห์กลุ่มผู้ใช้งาน
 * - รองรับ Google Analytics 4 (GA4) อัตโนมัติหากระบุ googleAnalyticsId ใน SITE_CONFIG
 */
function initVisitorAnalytics() {
  // 1. Optional Google Analytics 4 (gtag.js)
  if (SITE_CONFIG.googleAnalyticsId && SITE_CONFIG.googleAnalyticsId.startsWith("G-")) {
    const gaScript = document.createElement("script");
    gaScript.async = true;
    gaScript.src = `https://www.googletagmanager.com/gtag/js?id=${SITE_CONFIG.googleAnalyticsId}`;
    document.head.appendChild(gaScript);
    window.dataLayer = window.dataLayer || [];
    function gtag() {
      window.dataLayer.push(arguments);
    }
    gtag("js", new Date());
    gtag("config", SITE_CONFIG.googleAnalyticsId);
  }

  // 2. Detect Client Environment (OS / Browser)
  const envEl = document.getElementById("stat-client-env");
  if (envEl) {
    const ua = navigator.userAgent || "";
    let os = "Desktop";
    if (/Windows/i.test(ua)) os = "Windows";
    else if (/Linux/i.test(ua) && !/Android/i.test(ua)) os = "Linux";
    else if (/Mac OS X/i.test(ua) && !/iPhone|iPad/i.test(ua)) os = "macOS";
    else if (/Android/i.test(ua)) os = "Android";
    else if (/iPhone|iPad/i.test(ua)) os = "iOS";

    let browser = "Browser";
    if (/Edg\//i.test(ua)) browser = "Edge";
    else if (/Chrome\//i.test(ua)) browser = "Chrome";
    else if (/Firefox\//i.test(ua)) browser = "Firefox";
    else if (/Safari\//i.test(ua)) browser = "Safari";

    envEl.textContent = `${os} • ${browser}`;
  }

  const ns = SITE_CONFIG.counterNamespace || "pksofttech-uplc-studio";
  const abacusBase = `https://abacus.jasoncameron.dev`;

  // 3. Page Views Counter (hit once per browser session, otherwise get)
  const viewsEl = document.getElementById("stat-page-views");
  const footerViewsEl = document.getElementById("footer-visit-count");
  const sessionKey = "uplc_visited_session";
  const alreadyVisited = sessionStorage.getItem(sessionKey);
  const viewEndpoint = alreadyVisited
    ? `${abacusBase}/get/${ns}/visits`
    : `${abacusBase}/hit/${ns}/visits`;

  fetch(viewEndpoint)
    .then((res) => (res.ok ? res.json() : Promise.reject(res.status)))
    .then((data) => {
      if (!alreadyVisited) sessionStorage.setItem(sessionKey, "1");
      const count = Number(data?.value || 1).toLocaleString();
      if (viewsEl) viewsEl.textContent = count;
      if (footerViewsEl) footerViewsEl.textContent = count;
    })
    .catch(() => {
      if (viewsEl) viewsEl.textContent = "1+";
      if (footerViewsEl) footerViewsEl.textContent = "1+";
    });

  // 4. GitHub Repository Stars & Releases Download Statistics
  const starsEl = document.getElementById("stat-gh-stars");
  const totalDlEl = document.getElementById("stat-total-downloads");
  const winDlEl = document.getElementById("stat-dl-win");
  const linDlEl = document.getElementById("stat-dl-lin");
  const fwDlEl = document.getElementById("stat-dl-fw");

  if (SITE_CONFIG.githubApiRepo) {
    // Fetch Repo Stars & Forks
    fetch(`https://api.github.com/repos/${SITE_CONFIG.githubApiRepo}`)
      .then((r) => (r.ok ? r.json() : null))
      .then((repo) => {
        if (repo && starsEl) {
          const stars = Number(repo.stargazers_count || 0);
          const forks = Number(repo.forks_count || 0);
          starsEl.textContent = `${stars.toLocaleString()} ★ / ${forks} ⑂`;
        }
      })
      .catch(() => {
        if (starsEl) starsEl.textContent = "GitHub";
      });

    // Fetch Per-Platform Download Counts from GitHub Releases
    fetch(`https://api.github.com/repos/${SITE_CONFIG.githubApiRepo}/releases`)
      .then((r) => (r.ok ? r.json() : []))
      .then((releases) => {
        let winCount = 0;
        let linCount = 0;
        let fwCount = 0;
        let totalCount = 0;

        if (Array.isArray(releases)) {
          releases.forEach((rel) => {
            (rel.assets || []).forEach((asset) => {
              const name = (asset.name || "").toLowerCase();
              const dl = Number(asset.download_count || 0);
              totalCount += dl;
              if (name.includes("windows") || name.endsWith(".zip")) {
                winCount += dl;
              } else if (name.includes("linux") || name.endsWith(".tar.gz")) {
                linCount += dl;
              } else if (name.endsWith(".uf2")) {
                fwCount += dl;
              }
            });
          });
        }

        if (totalDlEl) totalDlEl.textContent = totalCount.toLocaleString();
        if (winDlEl) winDlEl.textContent = `${winCount.toLocaleString()} downloads`;
        if (linDlEl) linDlEl.textContent = `${linCount.toLocaleString()} downloads`;
        if (fwDlEl) fwDlEl.textContent = `${fwCount.toLocaleString()} downloads`;
      })
      .catch(() => {
        if (totalDlEl) totalDlEl.textContent = "0";
      });
  }

  // 5. Track Download Button Clicks in Abacus for Real-Time Click Analytics
  const trackDownloadClick = (buttonId, keyName, badgeEl) => {
    const btn = document.getElementById(buttonId);
    if (!btn) return;
    btn.addEventListener("click", () => {
      fetch(`${abacusBase}/hit/${ns}/${keyName}`).catch(() => {});
      if (badgeEl) {
        const currentMatch = (badgeEl.textContent || "0").match(/\d+/);
        const nextVal = currentMatch ? parseInt(currentMatch[0], 10) + 1 : 1;
        badgeEl.textContent = `${nextVal.toLocaleString()} downloads`;
      }
      if (totalDlEl) {
        const totalMatch = (totalDlEl.textContent || "0").replace(/,/g, "").match(/\d+/);
        if (totalMatch) {
          totalDlEl.textContent = (parseInt(totalMatch[0], 10) + 1).toLocaleString();
        }
      }
    });
  };

  trackDownloadClick("dl-win-link", "dl-windows", winDlEl);
  trackDownloadClick("dl-lin-link", "dl-linux", linDlEl);
  trackDownloadClick("dl-fw-link", "dl-uf2", fwDlEl);
}
