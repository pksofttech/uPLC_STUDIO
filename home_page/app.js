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
  windowsDownloadUrl: "../releases/laduino-plc-v0.1.0-x86_64-pc-windows-gnu.zip",
  linuxDownloadUrl: "../releases/laduino-plc-v0.1.0-x86_64-unknown-linux-gnu.tar.gz",
  firmwareUf2Url: "../releases/uplc-rp2040-embassy.uf2",

  // ข้อมูลรับบริจาค / Support & Donate
  promptPayQrImage: "assets/promptpay-qr.svg",
  promptPayNameTh: "บรรณพต นุ่มนาค",
  promptPayNameEn: "บรรณพต นุ่มนาค",
  promptPayId: "3660600255351",
  githubSponsorsUrl: "https://github.com/sponsors/pksofttech",
  buyMeACoffeeUrl: "https://buymeacoffee.com/pksofttech"
};

/**
 * Bilingual Dictionary (TH / EN)
 */
const I18N = {
  th: {
    nav_features: "ฟีเจอร์เด่น",
    nav_getting_started: "เริ่มต้นใช้งาน",
    nav_hardware: "ฮาร์ดแวร์ & พิน",
    nav_download: "ดาวน์โหลด",
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
    hw_subtitle: "การแมปพอร์ตมาตรฐานสำหรับบอร์ด Raspberry Pi Pico (RP2040) และ Address ตามมาตรฐาน IEC 61131-3",
    th_region: "หมวดหมู่ (Region)",
    th_iec: "IEC 61131-3 Address",
    th_alias: "PLC Alias",
    th_pin: "พินฮาร์ดแวร์ RP2040",
    th_desc: "รายละเอียดการใช้งาน",
    hw_r1_desc: "อินพุตดิจิทัล 8 ช่อง (Active-High / Pull-down) พร้อมระบบ Force I/O",
    hw_r2_desc: "เอาต์พุตดิจิทัล 8 ช่อง (Y0 = GP25 LED บนบอร์ด Pico, Y1..Y7 = GP8..GP14)",
    hw_r3_desc: "รีเลย์ช่วยภายในทั่วไป (เคลียร์ค่าเมื่อเริ่มรันใหม่)",
    hw_r4_desc: "รีเลย์ช่วยชนิดจำค่าเมื่อไฟดับ บันทึกลง RP2040 Flash อัตโนมัติ",
    hw_r5_desc: "รีจิสเตอร์ข้อมูล 16/32-bit สำหรับคำนวณ ค่านับ และงานทั่วไป",
    hw_r6_desc: "รีจิสเตอร์ข้อมูลชนิดจำค่าเมื่อไฟดับ (Retentive Flash Memory)",
    hw_r7_desc: "อนาล็อกอินพุต 12-bit (0..4095) และเซนเซอร์วัดอุณหภูมิภายในชิป (%IW3)",
    hw_r8_desc: "พอร์ตสื่อสาร RS-485 Half-Duplex สำหรับบล็อกคำสั่ง MB_MASTER",
    hw_r9_desc: "สัญญาณนาฬิกาพัลส์ 10ms..60s, First Scan (%SM1) และค่า Scan Time / อุณหภูมิ CPU (%SD6)",

    dl_tag: "DOWNLOAD RELEASES",
    dl_title: "ดาวน์โหลด LaduinoPLC & เฟิร์มแวร์",
    dl_subtitle: "ฟรีและโอเพนซอร์ส — ดาวน์โหลดเวอร์ชันล่าสุดได้โดยตรงจาก GitHub Releases",
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

    footer_copy: "พัฒนาโดย pksofttech.org • ซอฟต์แวร์ออกแบบและจำลองวงจร PLC มาตรฐาน IEC 61131-3",
    modal_close: "ปิดหน้าต่าง (Close)"
  },

  en: {
    nav_features: "Features",
    nav_getting_started: "Getting Started",
    nav_hardware: "Hardware & Pinout",
    nav_download: "Download",
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
    hw_title: "Standard Pinout & Memory Map (RP2040 Micro-VM)",
    hw_subtitle: "Default hardware pin assignments for Raspberry Pi Pico (RP2040) and IEC 61131-3 memory addressing.",
    th_region: "Memory Region",
    th_iec: "IEC 61131-3 Address",
    th_alias: "PLC Alias",
    th_pin: "RP2040 Hardware Pin",
    th_desc: "Description",
    hw_r1_desc: "8 Digital Inputs (Active-High / Pull-down) with real-time Force I/O support",
    hw_r2_desc: "8 Digital Outputs (Y0 = GP25 onboard Pico LED, Y1..Y7 = GP8..GP14)",
    hw_r3_desc: "General-purpose internal bit relays (cleared on PLC reset)",
    hw_r4_desc: "Retentive bit relays automatically backed up to RP2040 Flash",
    hw_r5_desc: "General-purpose signed 32-bit word registers for math & data",
    hw_r6_desc: "Retentive word registers backed up to non-volatile RP2040 Flash",
    hw_r7_desc: "12-bit Analog Inputs (0..4095) & onboard die temperature sensor (%IW3)",
    hw_r8_desc: "RS-485 Half-Duplex UART1 transceiver pins for MB_MASTER Modbus RTU",
    hw_r9_desc: "System clock pulses (10ms..60s), First Scan (%SM1), Scan Time & CPU Temp (%SD6)",

    dl_tag: "DOWNLOAD RELEASES",
    dl_title: "Download LaduinoPLC & Firmware",
    dl_subtitle: "Free & Open-Source — Get the latest pre-built packages directly from GitHub Releases.",
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
});
