#![windows_subsystem = "windows"]

use eframe::egui;
use rand::Rng;
use rdev::{listen, Event, EventType, Key, simulate, Button};
use std::sync::{Arc, Mutex, atomic::{AtomicBool, Ordering}};
use std::thread;
use std::time::{Duration, Instant};
use std::path::PathBuf;
use std::fs;
use serde::{Deserialize, Serialize};
use windows::Win32::{
    Foundation::{HWND, LPARAM, BOOL, RECT, WPARAM},
    UI::WindowsAndMessaging::{
        EnumWindows, GetClientRect, GetWindowTextLengthW, GetWindowTextW, IsWindowVisible,
        PostMessageW, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_RBUTTONDOWN, WM_RBUTTONUP,
        WM_KEYDOWN, WM_KEYUP,
    },
};

#[derive(Clone)]
struct PyladeClickerApp {
    clicking: Arc<AtomicBool>,
    click_mode: Arc<Mutex<ClickMode>>,
    click_type: Arc<Mutex<ClickType>>,
    target_window: Arc<Mutex<Option<String>>>,
    windows: Vec<String>,
    _last_click_time: Arc<Mutex<Instant>>,
    _humanized_delay: Arc<Mutex<Duration>>,
    normal_delay: Arc<Mutex<Duration>>,
    cps: Arc<Mutex<f32>>,
    hotkey: Arc<Mutex<Vec<Key>>>,
    capturing_hotkey: Arc<AtomicBool>,
    listening_text: Arc<Mutex<String>>,
    current_combination: Arc<Mutex<Vec<Key>>>,
    last_window_refresh: Arc<Mutex<Instant>>,
    is_holding: Arc<AtomicBool>,
}

#[derive(Clone, PartialEq)]
enum ClickMode {
    Click,
    Hold,
    Humanized,
}

#[derive(Clone, PartialEq)]
enum ClickType {
    LeftClick,
    RightClick,
    Space,
}

#[derive(Serialize, Deserialize, Clone)]
struct AppConfig {
    hotkey: Vec<String>,
    click_mode: String,
    click_type: String,
    normal_delay_ms: u64,
    cps: f32,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            hotkey: vec!["F6".to_string()],
            click_mode: "Click".to_string(),
            click_type: "LeftClick".to_string(),
            normal_delay_ms: 1000,
            cps: 10.0,
        }
    }
}

impl Default for PyladeClickerApp {
    fn default() -> Self {
        let config = load_config();
        
        let hotkey_keys: Vec<Key> = config.hotkey.iter()
            .filter_map(|s| string_to_key(s))
            .collect();
        
        let click_mode = match config.click_mode.as_str() {
            "Hold" => ClickMode::Hold,
            "Humanized" => ClickMode::Humanized,
            _ => ClickMode::Click,
        };
        
        let click_type = match config.click_type.as_str() {
            "RightClick" => ClickType::RightClick,
            "Space" => ClickType::Space,
            _ => ClickType::LeftClick,
        };
        
        let mut app = Self {
            clicking: Arc::new(AtomicBool::new(false)),
            click_mode: Arc::new(Mutex::new(click_mode)),
            click_type: Arc::new(Mutex::new(click_type)),
            target_window: Arc::new(Mutex::new(None)),
            windows: Vec::new(),
            _last_click_time: Arc::new(Mutex::new(Instant::now())),
            _humanized_delay: Arc::new(Mutex::new(Duration::from_millis(100))),
            normal_delay: Arc::new(Mutex::new(Duration::from_millis(config.normal_delay_ms))),
            cps: Arc::new(Mutex::new(config.cps)),
            hotkey: Arc::new(Mutex::new(if hotkey_keys.is_empty() { vec![Key::F6] } else { hotkey_keys })),
            capturing_hotkey: Arc::new(AtomicBool::new(false)),
            listening_text: Arc::new(Mutex::new(String::new())),
            current_combination: Arc::new(Mutex::new(Vec::new())),
            last_window_refresh: Arc::new(Mutex::new(Instant::now())),
            is_holding: Arc::new(AtomicBool::new(false)),
        };
        
        app.refresh_windows();
        
        app
    }
}

fn vk_to_key(vk: u32) -> Option<Key> {
    match vk {
        0x70 => Some(Key::F1),
        0x71 => Some(Key::F2),
        0x72 => Some(Key::F3),
        0x73 => Some(Key::F4),
        0x74 => Some(Key::F5),
        0x75 => Some(Key::F6),
        0x76 => Some(Key::F7),
        0x77 => Some(Key::F8),
        0x78 => Some(Key::F9),
        0x79 => Some(Key::F10),
        0x7A => Some(Key::F11),
        0x7B => Some(Key::F12),
        0x20 => Some(Key::Space),
        0x0D => Some(Key::Return),
        0x1B => Some(Key::Escape),
        0x09 => Some(Key::Tab),
        0x14 => Some(Key::CapsLock),
        0xA0 => Some(Key::ShiftLeft),
        0xA1 => Some(Key::ShiftRight),
        0xA2 => Some(Key::ControlLeft),
        0xA3 => Some(Key::ControlRight),
        0x12 => Some(Key::Alt),
        0xA5 => Some(Key::AltGr),
        _ => None,
    }
}

fn key_to_string(key: &Key) -> String {
    match key {
        Key::F1 => "F1".to_string(),
        Key::F2 => "F2".to_string(),
        Key::F3 => "F3".to_string(),
        Key::F4 => "F4".to_string(),
        Key::F5 => "F5".to_string(),
        Key::F6 => "F6".to_string(),
        Key::F7 => "F7".to_string(),
        Key::F8 => "F8".to_string(),
        Key::F9 => "F9".to_string(),
        Key::F10 => "F10".to_string(),
        Key::F11 => "F11".to_string(),
        Key::F12 => "F12".to_string(),
        
        Key::Home => "Home".to_string(),
        Key::End => "End".to_string(),
        Key::PageUp => "Page Up".to_string(),
        Key::PageDown => "Page Down".to_string(),
        Key::Insert => "Insert".to_string(),
        Key::Delete => "Delete".to_string(),
        Key::UpArrow => "Up".to_string(),
        Key::DownArrow => "Down".to_string(),
        Key::LeftArrow => "Left".to_string(),
        Key::RightArrow => "Right".to_string(),
        
        Key::Space => "Space".to_string(),
        Key::Return => "Enter".to_string(),
        Key::Escape => "Escape".to_string(),
        Key::Tab => "Tab".to_string(),
        Key::Backspace => "Backspace".to_string(),
        Key::CapsLock => "Caps Lock".to_string(),
        
        Key::ShiftLeft => "Left Shift".to_string(),
        Key::ShiftRight => "Right Shift".to_string(),
        Key::ControlLeft => "Left Ctrl".to_string(),
        Key::ControlRight => "Right Ctrl".to_string(),
        Key::Alt => "Alt".to_string(),
        Key::AltGr => "Alt Gr".to_string(),
        
        _ => format!("{:?}", key),
    }
}

fn combination_to_string(combination: &[Key]) -> String {
    if combination.is_empty() {
        return "Press keys...".to_string();
    }
    
    let key_strings: Vec<String> = combination.iter().map(key_to_string).collect();
    key_strings.join(" + ")
}

fn start_hotkey_toggle_listener(hotkey: Arc<Mutex<Vec<Key>>>, clicking: Arc<AtomicBool>) {
    std::thread::spawn(move || {
        thread::sleep(Duration::from_millis(200));
        let callback = move |event: Event| {
            if let EventType::KeyPress(key) = event.event_type {
                let current_hotkey = hotkey.lock().unwrap();
                
                if current_hotkey.len() == 1 {
                    if let Some(&hotkey_key) = current_hotkey.first() {
                        if key == hotkey_key {
                            clicking.store(!clicking.load(Ordering::SeqCst), Ordering::SeqCst);
                        }
                    }
                }
                else if current_hotkey.contains(&key) && current_hotkey.len() > 1 {
                    clicking.store(!clicking.load(Ordering::SeqCst), Ordering::SeqCst);
                }
            }
        };
        
        if let Err(error) = listen(callback) {
            eprintln!("Failed to start hotkey toggle listener: {:?}", error);
        }
    });
}

fn start_clicking_thread(
    clicking: Arc<AtomicBool>,
    click_mode: Arc<Mutex<ClickMode>>,
    click_type: Arc<Mutex<ClickType>>,
    target_window: Arc<Mutex<Option<String>>>,
    _last_click_time: Arc<Mutex<Instant>>,
    _humanized_delay: Arc<Mutex<Duration>>,
    normal_delay: Arc<Mutex<Duration>>,
    cps: Arc<Mutex<f32>>,
    is_holding: Arc<AtomicBool>,
) {
    std::thread::spawn(move || {
        let mut rng = rand::thread_rng();
        
        loop {
            if clicking.load(Ordering::SeqCst) {
                let mode = click_mode.lock().unwrap().clone();
                let click_type = click_type.lock().unwrap().clone();
                let target = target_window.lock().unwrap().clone();
                
                match mode {
                    ClickMode::Click => {
                        let delay = *normal_delay.lock().unwrap();
                        perform_click(&click_type, &target);
                        thread::sleep(delay);
                    }
                    ClickMode::Hold => {
                        if !is_holding.load(Ordering::SeqCst) {
                            perform_hold(&click_type, &target);
                            is_holding.store(true, Ordering::SeqCst);
                        }
                        thread::sleep(Duration::from_millis(10));
                    }
                    ClickMode::Humanized => {
                        let cps_value = *cps.lock().unwrap();
                        
                        if cps_value > 50.0 {
                            drag_click_burst(&target, cps_value, &mut rng, &click_type);
                            
                            let break_time = Duration::from_millis(rng.gen_range(450..=550));
                            thread::sleep(break_time);
                        } else {
                            perform_click(&click_type, &target);
                            
                            let delay = calculate_humanized_delay(cps_value, &mut rng);
                            thread::sleep(delay);
                        }
                    }
                }
            } else {
                if is_holding.load(Ordering::SeqCst) {
                    let target = target_window.lock().unwrap().clone();
                    let click_type = click_type.lock().unwrap().clone();
                    perform_release(&click_type, &target);
                    is_holding.store(false, Ordering::SeqCst);
                }
                thread::sleep(Duration::from_millis(10));
            }
        }
    });
}

fn calculate_humanized_delay(cps: f32, rng: &mut impl rand::Rng) -> Duration {
    let base_delay_ms = 1000.0 / cps;
    let variation = if cps > 20.0 {
        rng.gen_range(-3.0..=3.0)
    } else if cps > 10.0 {
        rng.gen_range(-5.0..=5.0)
    } else {
        rng.gen_range(-10.0..=10.0)
    };
    let final_delay = (base_delay_ms + variation).max(1.0);
    
    
    Duration::from_millis(final_delay as u64)
}

fn drag_click_burst(target: &Option<String>, target_cps: f32, rng: &mut impl rand::Rng, click_type: &ClickType) {
    let base_burst_size = (target_cps * 0.5) as usize;
    let burst_count = rng.gen_range((base_burst_size.saturating_sub(5))..=(base_burst_size + 5));
    
    let burst_delay = Duration::from_micros(rng.gen_range(500..=1500));
    
    for i in 0..burst_count {
        perform_click(click_type, target);
        
        if i < burst_count - 1 {
            thread::sleep(burst_delay);
        }
    }
}

fn perform_click(click_type: &ClickType, target: &Option<String>) {
    match click_type {
        ClickType::LeftClick => {
            if let Some(ref window_title) = target {
                click_target_window(window_title);
            } else {
                simulate(&EventType::ButtonPress(Button::Left)).unwrap();
                thread::sleep(Duration::from_millis(1));
                simulate(&EventType::ButtonRelease(Button::Left)).unwrap();
            }
        }
        ClickType::RightClick => {
            if let Some(ref window_title) = target {
                right_click_target_window(window_title);
            } else {
                simulate(&EventType::ButtonPress(Button::Right)).unwrap();
                thread::sleep(Duration::from_millis(1));
                simulate(&EventType::ButtonRelease(Button::Right)).unwrap();
            }
        }
        ClickType::Space => {
            if let Some(ref window_title) = target {
                space_target_window(window_title);
            } else {
                simulate(&EventType::KeyPress(Key::Space)).unwrap();
                thread::sleep(Duration::from_millis(1));
                simulate(&EventType::KeyRelease(Key::Space)).unwrap();
            }
        }
    }
}

fn perform_hold(click_type: &ClickType, target: &Option<String>) {
    match click_type {
        ClickType::LeftClick => {
            if let Some(ref window_title) = target {
                hold_target_window(window_title);
            } else {
                simulate(&EventType::ButtonPress(Button::Left)).unwrap();
            }
        }
        ClickType::RightClick => {
            if let Some(ref window_title) = target {
                right_hold_target_window(window_title);
            } else {
                simulate(&EventType::ButtonPress(Button::Right)).unwrap();
            }
        }
        ClickType::Space => {
            if let Some(ref window_title) = target {
                space_hold_target_window(window_title);
            } else {
                simulate(&EventType::KeyPress(Key::Space)).unwrap();
            }
        }
    }
}

fn perform_release(click_type: &ClickType, target: &Option<String>) {
    match click_type {
        ClickType::LeftClick => {
            if let Some(ref window_title) = target {
                release_target_window(window_title);
            } else {
                simulate(&EventType::ButtonRelease(Button::Left)).unwrap();
            }
        }
        ClickType::RightClick => {
            if let Some(ref window_title) = target {
                right_release_target_window(window_title);
            } else {
                simulate(&EventType::ButtonRelease(Button::Right)).unwrap();
            }
        }
        ClickType::Space => {
            if let Some(ref window_title) = target {
                space_release_target_window(window_title);
            } else {
                simulate(&EventType::KeyRelease(Key::Space)).unwrap();
            }
        }
    }
}

fn click_target_window(window_title: &str) {
    unsafe {
        let mut found_hwnd = None;
        let mut window_data = Vec::new();
        
        EnumWindows(
            Some(enum_windows_proc),
            LPARAM(&mut window_data as *mut Vec<(HWND, String)> as isize),
        );
        
        for (hwnd, title) in window_data.iter() {
            if title == window_title {
                found_hwnd = Some(*hwnd);
                break;
            }
        }
        
        if let Some(hwnd) = found_hwnd {
            let mut client_rect = RECT::default();
            GetClientRect(hwnd, &mut client_rect);
            let client_x = (client_rect.left + client_rect.right) / 2;
            let client_y = (client_rect.top + client_rect.bottom) / 2;
            let lparam = ((client_y as u32) << 16) | (client_x as u32);
            
            PostMessageW(hwnd, WM_LBUTTONDOWN, WPARAM(1), LPARAM(lparam as isize));
            thread::sleep(Duration::from_millis(1));
            PostMessageW(hwnd, WM_LBUTTONUP, WPARAM(0), LPARAM(lparam as isize));
        }
    }
}

fn right_click_target_window(window_title: &str) {
    unsafe {
        let mut found_hwnd = None;
        let mut window_data = Vec::new();
        
        EnumWindows(
            Some(enum_windows_proc),
            LPARAM(&mut window_data as *mut Vec<(HWND, String)> as isize),
        );
        
        for (hwnd, title) in window_data.iter() {
            if title == window_title {
                found_hwnd = Some(*hwnd);
                break;
            }
        }
        
        if let Some(hwnd) = found_hwnd {
            let mut client_rect = RECT::default();
            GetClientRect(hwnd, &mut client_rect);
            let client_x = (client_rect.left + client_rect.right) / 2;
            let client_y = (client_rect.top + client_rect.bottom) / 2;
            let lparam = ((client_y as u32) << 16) | (client_x as u32);
            
            PostMessageW(hwnd, WM_RBUTTONDOWN, WPARAM(1), LPARAM(lparam as isize));
            thread::sleep(Duration::from_millis(1));
            PostMessageW(hwnd, WM_RBUTTONUP, WPARAM(0), LPARAM(lparam as isize));
        }
    }
}

fn space_target_window(window_title: &str) {
    unsafe {
        let mut found_hwnd = None;
        let mut window_data = Vec::new();
        
        EnumWindows(
            Some(enum_windows_proc),
            LPARAM(&mut window_data as *mut Vec<(HWND, String)> as isize),
        );
        
        for (hwnd, title) in window_data.iter() {
            if title == window_title {
                found_hwnd = Some(*hwnd);
                break;
            }
        }
        
        if let Some(hwnd) = found_hwnd {
            PostMessageW(hwnd, WM_KEYDOWN, WPARAM(0x20), LPARAM(0));
            thread::sleep(Duration::from_millis(1));
            PostMessageW(hwnd, WM_KEYUP, WPARAM(0x20), LPARAM(0));
        }
    }
}

fn hold_target_window(window_title: &str) {
    unsafe {
        let mut found_hwnd = None;
        let mut window_data = Vec::new();
        
        EnumWindows(
            Some(enum_windows_proc),
            LPARAM(&mut window_data as *mut Vec<(HWND, String)> as isize),
        );
        
        for (hwnd, title) in window_data.iter() {
            if title == window_title {
                found_hwnd = Some(*hwnd);
                break;
            }
        }
        
        if let Some(hwnd) = found_hwnd {
            let mut client_rect = RECT::default();
            GetClientRect(hwnd, &mut client_rect);
            let client_x = (client_rect.left + client_rect.right) / 2;
            let client_y = (client_rect.top + client_rect.bottom) / 2;
            let lparam = ((client_y as u32) << 16) | (client_x as u32);
            
            PostMessageW(hwnd, WM_LBUTTONDOWN, WPARAM(1), LPARAM(lparam as isize));
        }
    }
}

fn release_target_window(window_title: &str) {
    unsafe {
        let mut found_hwnd = None;
        let mut window_data = Vec::new();
        
        EnumWindows(
            Some(enum_windows_proc),
            LPARAM(&mut window_data as *mut Vec<(HWND, String)> as isize),
        );
        
        for (hwnd, title) in window_data.iter() {
            if title == window_title {
                found_hwnd = Some(*hwnd);
                break;
            }
        }
        
        if let Some(hwnd) = found_hwnd {
            let mut client_rect = RECT::default();
            GetClientRect(hwnd, &mut client_rect);
            let client_x = (client_rect.left + client_rect.right) / 2;
            let client_y = (client_rect.top + client_rect.bottom) / 2;
            let lparam = ((client_y as u32) << 16) | (client_x as u32);
            
            PostMessageW(hwnd, WM_LBUTTONUP, WPARAM(0), LPARAM(lparam as isize));
        }
    }
}

fn right_hold_target_window(window_title: &str) {
    unsafe {
        let mut found_hwnd = None;
        let mut window_data = Vec::new();
        
        EnumWindows(
            Some(enum_windows_proc),
            LPARAM(&mut window_data as *mut Vec<(HWND, String)> as isize),
        );
        
        for (hwnd, title) in window_data.iter() {
            if title == window_title {
                found_hwnd = Some(*hwnd);
                break;
            }
        }
        
        if let Some(hwnd) = found_hwnd {
            let mut client_rect = RECT::default();
            GetClientRect(hwnd, &mut client_rect);
            let client_x = (client_rect.left + client_rect.right) / 2;
            let client_y = (client_rect.top + client_rect.bottom) / 2;
            let lparam = ((client_y as u32) << 16) | (client_x as u32);
            
            PostMessageW(hwnd, WM_RBUTTONDOWN, WPARAM(1), LPARAM(lparam as isize));
        }
    }
}

fn right_release_target_window(window_title: &str) {
    unsafe {
        let mut found_hwnd = None;
        let mut window_data = Vec::new();
        
        EnumWindows(
            Some(enum_windows_proc),
            LPARAM(&mut window_data as *mut Vec<(HWND, String)> as isize),
        );
        
        for (hwnd, title) in window_data.iter() {
            if title == window_title {
                found_hwnd = Some(*hwnd);
                break;
            }
        }
        
        if let Some(hwnd) = found_hwnd {
            let mut client_rect = RECT::default();
            GetClientRect(hwnd, &mut client_rect);
            let client_x = (client_rect.left + client_rect.right) / 2;
            let client_y = (client_rect.top + client_rect.bottom) / 2;
            let lparam = ((client_y as u32) << 16) | (client_x as u32);
            
            PostMessageW(hwnd, WM_RBUTTONUP, WPARAM(0), LPARAM(lparam as isize));
        }
    }
}

fn space_hold_target_window(window_title: &str) {
    unsafe {
        let mut found_hwnd = None;
        let mut window_data = Vec::new();
        
        EnumWindows(
            Some(enum_windows_proc),
            LPARAM(&mut window_data as *mut Vec<(HWND, String)> as isize),
        );
        
        for (hwnd, title) in window_data.iter() {
            if title == window_title {
                found_hwnd = Some(*hwnd);
                break;
            }
        }
        
        if let Some(hwnd) = found_hwnd {
            PostMessageW(hwnd, WM_KEYDOWN, WPARAM(0x20), LPARAM(0));
        }
    }
}

fn space_release_target_window(window_title: &str) {
    unsafe {
        let mut found_hwnd = None;
        let mut window_data = Vec::new();
        
        EnumWindows(
            Some(enum_windows_proc),
            LPARAM(&mut window_data as *mut Vec<(HWND, String)> as isize),
        );
        
        for (hwnd, title) in window_data.iter() {
            if title == window_title {
                found_hwnd = Some(*hwnd);
                break;
            }
        }
        
        if let Some(hwnd) = found_hwnd {
            PostMessageW(hwnd, WM_KEYUP, WPARAM(0x20), LPARAM(0));
        }
    }
}











impl PyladeClickerApp {
    fn save_current_config(&self) {
        let hotkey_strings: Vec<String> = self.hotkey.lock().unwrap().iter()
            .map(|k| key_to_string(k))
            .collect();
        
        let click_mode_str = match *self.click_mode.lock().unwrap() {
            ClickMode::Hold => "Hold",
            ClickMode::Humanized => "Humanized",
            _ => "Click",
        };
        
        let click_type_str = match *self.click_type.lock().unwrap() {
            ClickType::RightClick => "RightClick",
            ClickType::Space => "Space",
            _ => "LeftClick",
        };
        
        let config = AppConfig {
            hotkey: hotkey_strings,
            click_mode: click_mode_str.to_string(),
            click_type: click_type_str.to_string(),
            normal_delay_ms: self.normal_delay.lock().unwrap().as_millis() as u64,
            cps: *self.cps.lock().unwrap(),
        };
        
        save_config(&config);
    }
}

impl eframe::App for PyladeClickerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let should_refresh = {
            let now = Instant::now();
            let last_refresh = self.last_window_refresh.lock().unwrap();
            now.duration_since(*last_refresh) >= Duration::from_secs(2)
        };
        
        if should_refresh {
            self.refresh_windows();
            *self.last_window_refresh.lock().unwrap() = Instant::now();
        }
        
        if !self.capturing_hotkey.load(Ordering::SeqCst) {
            ctx.input(|i| {
                let current_hotkey = self.hotkey.lock().unwrap();
                
                if current_hotkey.len() == 1 {
                    if let Some(&hotkey_key) = current_hotkey.first() {
                        let gui_key = match hotkey_key {
                            Key::F1 => Some(egui::Key::F1),
                            Key::F2 => Some(egui::Key::F2),
                            Key::F3 => Some(egui::Key::F3),
                            Key::F4 => Some(egui::Key::F4),
                            Key::F5 => Some(egui::Key::F5),
                            Key::F6 => Some(egui::Key::F6),
                            Key::F7 => Some(egui::Key::F7),
                            Key::F8 => Some(egui::Key::F8),
                            Key::F9 => Some(egui::Key::F9),
                            Key::F10 => Some(egui::Key::F10),
                            Key::F11 => Some(egui::Key::F11),
                            Key::F12 => Some(egui::Key::F12),
                            Key::Space => Some(egui::Key::Space),
                            Key::Return => Some(egui::Key::Enter),
                            Key::Escape => Some(egui::Key::Escape),
                            Key::Tab => Some(egui::Key::Tab),
                            Key::Home => Some(egui::Key::Home),
                            Key::End => Some(egui::Key::End),
                            Key::PageUp => Some(egui::Key::PageUp),
                            Key::PageDown => Some(egui::Key::PageDown),
                            Key::Insert => Some(egui::Key::Insert),
                            Key::Delete => Some(egui::Key::Delete),
                            Key::UpArrow => Some(egui::Key::ArrowUp),
                            Key::DownArrow => Some(egui::Key::ArrowDown),
                            Key::LeftArrow => Some(egui::Key::ArrowLeft),
                            Key::RightArrow => Some(egui::Key::ArrowRight),
                            Key::Backspace => Some(egui::Key::Backspace),
                            _ => None,
                        };
                        
                        if let Some(gui_key) = gui_key {
                            if i.key_pressed(gui_key) {
                                self.clicking.store(!self.clicking.load(Ordering::SeqCst), Ordering::SeqCst);
                            }
                        }
                    }
                }
                else if current_hotkey.len() > 1 {
                    for &hotkey_key in current_hotkey.iter() {
                        let gui_key = match hotkey_key {
                            Key::F1 => Some(egui::Key::F1),
                            Key::F2 => Some(egui::Key::F2),
                            Key::F3 => Some(egui::Key::F3),
                            Key::F4 => Some(egui::Key::F4),
                            Key::F5 => Some(egui::Key::F5),
                            Key::F6 => Some(egui::Key::F6),
                            Key::F7 => Some(egui::Key::F7),
                            Key::F8 => Some(egui::Key::F8),
                            Key::F9 => Some(egui::Key::F9),
                            Key::F10 => Some(egui::Key::F10),
                            Key::F11 => Some(egui::Key::F11),
                            Key::F12 => Some(egui::Key::F12),
                            Key::Space => Some(egui::Key::Space),
                            Key::Return => Some(egui::Key::Enter),
                            Key::Escape => Some(egui::Key::Escape),
                            Key::Tab => Some(egui::Key::Tab),
                            Key::Home => Some(egui::Key::Home),
                            Key::End => Some(egui::Key::End),
                            Key::PageUp => Some(egui::Key::PageUp),
                            Key::PageDown => Some(egui::Key::PageDown),
                            Key::Insert => Some(egui::Key::Insert),
                            Key::Delete => Some(egui::Key::Delete),
                            Key::UpArrow => Some(egui::Key::ArrowUp),
                            Key::DownArrow => Some(egui::Key::ArrowDown),
                            Key::LeftArrow => Some(egui::Key::ArrowLeft),
                            Key::RightArrow => Some(egui::Key::ArrowRight),
                            Key::Backspace => Some(egui::Key::Backspace),
                            _ => None,
                        };
                        
                        if let Some(gui_key) = gui_key {
                            if i.key_pressed(gui_key) {
                                self.clicking.store(!self.clicking.load(Ordering::SeqCst), Ordering::SeqCst);
                                break;
                            }
                        }
                    }
                }
            });
        }
        
        if self.capturing_hotkey.load(Ordering::SeqCst) {
            ctx.input(|i| {
                let modifiers = i.modifiers;
                
                let mut current_combo = Vec::new();
                
                if modifiers.shift {
                    current_combo.push(Key::ShiftLeft);
                }
                if modifiers.ctrl {
                    current_combo.push(Key::ControlLeft);
                }
                if modifiers.alt {
                    current_combo.push(Key::Alt);
                }
                
                for key in [
                    (egui::Key::F1, Key::F1),
                    (egui::Key::F2, Key::F2),
                    (egui::Key::F3, Key::F3),
                    (egui::Key::F4, Key::F4),
                    (egui::Key::F5, Key::F5),
                    (egui::Key::F6, Key::F6),
                    (egui::Key::F7, Key::F7),
                    (egui::Key::F8, Key::F8),
                    (egui::Key::F9, Key::F9),
                    (egui::Key::F10, Key::F10),
                    (egui::Key::F11, Key::F11),
                    (egui::Key::F12, Key::F12),
                    (egui::Key::Home, Key::Home),
                    (egui::Key::End, Key::End),
                    (egui::Key::PageUp, Key::PageUp),
                    (egui::Key::PageDown, Key::PageDown),
                    (egui::Key::Insert, Key::Insert),
                    (egui::Key::Delete, Key::Delete),
                    (egui::Key::ArrowUp, Key::UpArrow),
                    (egui::Key::ArrowDown, Key::DownArrow),
                    (egui::Key::ArrowLeft, Key::LeftArrow),
                    (egui::Key::ArrowRight, Key::RightArrow),
                    (egui::Key::Space, Key::Space),
                    (egui::Key::Enter, Key::Return),
                    (egui::Key::Escape, Key::Escape),
                    (egui::Key::Tab, Key::Tab),
                    (egui::Key::Backspace, Key::Backspace),
                ] {
                    if i.key_down(key.0) && !current_combo.contains(&key.1) {
                        current_combo.push(key.1);
                    }
                }
                
                *self.current_combination.lock().unwrap() = current_combo.clone();
                
                if current_combo.len() >= 2 {
                    *self.hotkey.lock().unwrap() = current_combo;
                    *self.listening_text.lock().unwrap() = String::new();
                    self.capturing_hotkey.store(false, Ordering::SeqCst);
                    *self.current_combination.lock().unwrap() = Vec::new();
                    self.save_current_config();
                }
                
                for event in &i.events {
                    if let egui::Event::Key { key, pressed: false, .. } = event {
                        let rdev_key = match key {
                            egui::Key::F1 => Some(Key::F1),
                            egui::Key::F2 => Some(Key::F2),
                            egui::Key::F3 => Some(Key::F3),
                            egui::Key::F4 => Some(Key::F4),
                            egui::Key::F5 => Some(Key::F5),
                            egui::Key::F6 => Some(Key::F6),
                            egui::Key::F7 => Some(Key::F7),
                            egui::Key::F8 => Some(Key::F8),
                            egui::Key::F9 => Some(Key::F9),
                            egui::Key::F10 => Some(Key::F10),
                            egui::Key::F11 => Some(Key::F11),
                            egui::Key::F12 => Some(Key::F12),
                            egui::Key::Space => Some(Key::Space),
                            egui::Key::Enter => Some(Key::Return),
                            egui::Key::Escape => Some(Key::Escape),
                            egui::Key::Tab => Some(Key::Tab),
                            egui::Key::Home => Some(Key::Home),
                            egui::Key::End => Some(Key::End),
                            egui::Key::PageUp => Some(Key::PageUp),
                            egui::Key::PageDown => Some(Key::PageDown),
                            egui::Key::Insert => Some(Key::Insert),
                            egui::Key::Delete => Some(Key::Delete),
                            egui::Key::ArrowUp => Some(Key::UpArrow),
                            egui::Key::ArrowDown => Some(Key::DownArrow),
                            egui::Key::ArrowLeft => Some(Key::LeftArrow),
                            egui::Key::ArrowRight => Some(Key::RightArrow),
                            egui::Key::Backspace => Some(Key::Backspace),
                            _ => None,
                        };
                        
                        if let Some(rdev_key) = rdev_key {
                            *self.hotkey.lock().unwrap() = vec![rdev_key];
                            *self.listening_text.lock().unwrap() = String::new();
                            self.capturing_hotkey.store(false, Ordering::SeqCst);
                            *self.current_combination.lock().unwrap() = Vec::new();
                            self.save_current_config();
                            break;
                        }
                    }
                }
            });
        }
        
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Pylade Clicker");
            
            ui.separator();
            
            ui.horizontal(|ui| {
                ui.label("Status:");
                if self.clicking.load(Ordering::SeqCst) {
                    ui.colored_label(egui::Color32::GREEN, "CLICKING");
                } else {
                    ui.colored_label(egui::Color32::RED, "STOPPED");
                }
            });
            
            ui.horizontal(|ui| {
                if self.clicking.load(Ordering::SeqCst) {
                    if ui.button("Stop Clicking").clicked() {
                        self.clicking.store(false, Ordering::SeqCst);
                    }
                } else {
                    if ui.button("Start Clicking").clicked() {
                        self.clicking.store(true, Ordering::SeqCst);
                    }
                }
                });

            ui.separator();

            ui.horizontal(|ui| {
                ui.label("Hotkey:");
                
                let button_text = if self.capturing_hotkey.load(Ordering::SeqCst) {
                    let combination = self.current_combination.lock().unwrap();
                    combination_to_string(&*combination)
                } else {
                    let hotkey = self.hotkey.lock().unwrap();
                    combination_to_string(&*hotkey)
                };
                
                if ui.button(&button_text).clicked() {
                    self.capturing_hotkey.store(true, Ordering::SeqCst);
                    *self.listening_text.lock().unwrap() = "Press and hold keys, release to confirm...".to_string();
                    *self.current_combination.lock().unwrap() = Vec::new();
                }
            });
            
            {
                let listening_text = self.listening_text.lock().unwrap();
                if !listening_text.is_empty() {
                    ui.horizontal(|ui| {
                        ui.colored_label(egui::Color32::YELLOW, &*listening_text);
                        if ui.button("Cancel").clicked() {
                            self.capturing_hotkey.store(false, Ordering::SeqCst);
                            drop(listening_text);
                            *self.listening_text.lock().unwrap() = String::new();
                        }
                    });
                }
            }

            ui.separator();

            ui.horizontal(|ui| {
                ui.label("Click Mode:");
                let mut current_mode = self.click_mode.lock().unwrap().clone();
                let changed_click = ui.radio_value(&mut current_mode, ClickMode::Click, "Click").changed();
                let changed_hold = ui.radio_value(&mut current_mode, ClickMode::Hold, "Hold").changed();
                let changed_humanized = ui.radio_value(&mut current_mode, ClickMode::Humanized, "Humanized").changed();
                
                if changed_click || changed_hold || changed_humanized {
                    *self.click_mode.lock().unwrap() = current_mode;
                    self.save_current_config();
                }
            });
            
            ui.horizontal(|ui| {
                ui.label("Click Type:");
                let mut current_type = self.click_type.lock().unwrap().clone();
                let changed_left = ui.radio_value(&mut current_type, ClickType::LeftClick, "Left Click").changed();
                let changed_right = ui.radio_value(&mut current_type, ClickType::RightClick, "Right Click").changed();
                let changed_space = ui.radio_value(&mut current_type, ClickType::Space, "Space").changed();
                
                if changed_left || changed_right || changed_space {
                    *self.click_type.lock().unwrap() = current_type;
                    self.save_current_config();
                }
            });
            
            let current_mode = self.click_mode.lock().unwrap().clone();
            if current_mode == ClickMode::Click {
                ui.horizontal(|ui| {
                    ui.label("Delay (ms):");
                    let mut delay = self.normal_delay.lock().unwrap().as_millis() as f32;
                    if ui.add(egui::Slider::new(&mut delay, 1.0..=1000.0)).changed() {
                        *self.normal_delay.lock().unwrap() = Duration::from_millis(delay as u64);
                        self.save_current_config();
                    }
                });
            }
            
            if current_mode == ClickMode::Humanized {
                ui.horizontal(|ui| {
                    ui.label("CPS:");
                    let mut cps = *self.cps.lock().unwrap();
                    if ui.add(egui::Slider::new(&mut cps, 1.0..=100.0)).changed() {
                        *self.cps.lock().unwrap() = cps;
                        self.save_current_config();
                    }
                });
                
                if *self.cps.lock().unwrap() > 50.0 {
                    ui.label("Burst mode enabled for very high CPS");
                }
            }

            ui.separator();

            ui.horizontal(|ui| {
                if ui.button("Refresh Windows").clicked() {
                    self.refresh_windows();
                }
                
                if ui.button("Clear Target").clicked() {
                    *self.target_window.lock().unwrap() = None;
                }
            });
            
            if !self.windows.is_empty() {
                ui.label("Available Windows:");
                
                egui::Frame::none()
                    .stroke(egui::Stroke::new(1.0, egui::Color32::GRAY))
                    .inner_margin(egui::Margin::same(8.0))
                    .outer_margin(egui::Margin::same(4.0))
                    .show(ui, |ui| {
                        egui::ScrollArea::vertical().max_height(200.0).show(ui, |ui| {
                            for window in &self.windows {
                                let is_selected = {
                                    let target = self.target_window.lock().unwrap();
                                    target.as_ref() == Some(window)
                                };
                                if ui.selectable_label(is_selected, window).clicked() {
                                    *self.target_window.lock().unwrap() = Some(window.clone());
                        }
                    }
                });
                    });
            }
            
            {
                let target = self.target_window.lock().unwrap();
                if let Some(ref target_name) = *target {
                    ui.label(format!("Target: {}", target_name));
                }
            }
        });
        
        ctx.request_repaint_after(Duration::from_millis(16));
    }
}

impl PyladeClickerApp {
    fn refresh_windows(&mut self) {
        self.windows.clear();
        let mut window_data = Vec::new();
        
        unsafe {
            EnumWindows(
                Some(enum_windows_proc),
                LPARAM(&mut window_data as *mut Vec<(HWND, String)> as isize),
            );
        }
        
        self.windows = window_data.into_iter().map(|(_, title)| title).collect();
    }
}

unsafe extern "system" fn enum_windows_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
    if IsWindowVisible(hwnd).into() {
        let length = GetWindowTextLengthW(hwnd);
        if length > 0 {
            let mut buffer = vec![0u16; (length + 1) as usize];
            GetWindowTextW(hwnd, &mut buffer);
            let title = String::from_utf16_lossy(&buffer[..length as usize]);
            if !title.is_empty() && title != "Program Manager" {
                let window_data = &mut *(lparam.0 as *mut Vec<(HWND, String)>);
                window_data.push((hwnd, title));
            }
        }
    }
    BOOL::from(true)
}

fn get_config_path() -> PathBuf {
    let mut path = dirs::home_dir().unwrap_or_else(|| PathBuf::from("C:\\"));
    path.push("Documents");
    path.push("PyladeClicker");
    path.push("config.json");
    path
}

fn load_config() -> AppConfig {
    let config_path = get_config_path();
    
    if let Ok(config_data) = fs::read_to_string(&config_path) {
        if let Ok(config) = serde_json::from_str::<AppConfig>(&config_data) {
            return config;
        }
    }
    
    AppConfig::default()
}

fn save_config(config: &AppConfig) {
    let config_path = get_config_path();
    
    if let Some(parent) = config_path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    
    if let Ok(config_json) = serde_json::to_string_pretty(config) {
        let _ = fs::write(&config_path, config_json);
    }
}


fn string_to_key(s: &str) -> Option<Key> {
    match s {
        "F1" => Some(Key::F1),
        "F2" => Some(Key::F2),
        "F3" => Some(Key::F3),
        "F4" => Some(Key::F4),
        "F5" => Some(Key::F5),
        "F6" => Some(Key::F6),
        "F7" => Some(Key::F7),
        "F8" => Some(Key::F8),
        "F9" => Some(Key::F9),
        "F10" => Some(Key::F10),
        "F11" => Some(Key::F11),
        "F12" => Some(Key::F12),
        "Space" => Some(Key::Space),
        "Enter" => Some(Key::Return),
        "Escape" => Some(Key::Escape),
        "Tab" => Some(Key::Tab),
        "Home" => Some(Key::Home),
        "End" => Some(Key::End),
        "PageUp" => Some(Key::PageUp),
        "PageDown" => Some(Key::PageDown),
        "Insert" => Some(Key::Insert),
        "Delete" => Some(Key::Delete),
        "Up" => Some(Key::UpArrow),
        "Down" => Some(Key::DownArrow),
        "Left" => Some(Key::LeftArrow),
        "Right" => Some(Key::RightArrow),
        "Backspace" => Some(Key::Backspace),
        _ => None,
    }
}

fn load_icon_data() -> egui::IconData {
    let icon_data = include_bytes!("../icon.ico");
    
    let icon_dir = ico::IconDir::read(std::io::Cursor::new(icon_data)).unwrap();
    let image = icon_dir.entries().first().unwrap();
    let image_data = image.decode().unwrap();
    
    egui::IconData {
        rgba: image_data.rgba_data().to_vec(),
        width: image_data.width(),
        height: image_data.height(),
    }
}

fn main() {
    let app = PyladeClickerApp::default();
    let hotkey = Arc::clone(&app.hotkey);
    let clicking = Arc::clone(&app.clicking);
    let _capturing_hotkey = Arc::clone(&app.capturing_hotkey);
    let _listening_text = Arc::clone(&app.listening_text);
    let click_mode = Arc::clone(&app.click_mode);
    let click_type = Arc::clone(&app.click_type);
    let target_window = Arc::clone(&app.target_window);
    let _last_click_time = Arc::clone(&app._last_click_time);
    let _humanized_delay = Arc::clone(&app._humanized_delay);
    let normal_delay = Arc::clone(&app.normal_delay);
    let cps = Arc::clone(&app.cps);
    let is_holding = Arc::clone(&app.is_holding);
    
    start_hotkey_toggle_listener(hotkey.clone(), clicking.clone());
    
    start_clicking_thread(
        clicking.clone(),
        click_mode,
        click_type,
        target_window,
        _last_click_time,
        _humanized_delay,
        normal_delay,
        cps,
        is_holding,
    );

    let mut native_options = eframe::NativeOptions::default();
    native_options.viewport = native_options.viewport.with_icon(load_icon_data());
    let _ = eframe::run_native("Pylade Clicker", native_options, Box::new(|_cc| Box::new(app)));
}