use aegis_core::{Value, NativeFn};
use std::collections::HashMap;
use std::sync::Mutex;
use lazy_static::lazy_static;
use glfw::{Context, Glfw, PWindow, GlfwReceiver, WindowEvent}; // Note l'ajout de PWindow et GlfwReceiver

// --- GLOBAL STATE ---

struct GlfwState {
    context: Glfw,
    // CORRECTION TYPE : PWindow et GlfwReceiver
    windows: HashMap<usize, (PWindow, GlfwReceiver<(f64, WindowEvent)>)>,
    next_id: usize,
}

// --- LE HACK POUR LE SEND ---
// On crée un wrapper vide qui contient notre état
struct ThreadSafeState(GlfwState);

// On dit à Rust : "Je certifie que c'est safe d'envoyer ça entre threads"
// (C'est nécessaire car GLFW utilise des pointeurs C bruts *mut void)
unsafe impl Send for ThreadSafeState {}

lazy_static! {
    // On utilise notre wrapper ThreadSafeState
    static ref STATE: Mutex<Option<ThreadSafeState>> = Mutex::new(None);
}

// --- REGISTRATION ---

#[unsafe(no_mangle)]
pub extern "C" fn _aegis_register(map: &mut HashMap<String, NativeFn>) {
    map.insert("glfw_init".to_string(), glfw_init);
    map.insert("glfw_create_window".to_string(), glfw_create_window);
    map.insert("glfw_window_should_close".to_string(), glfw_window_should_close);
    map.insert("glfw_poll_events".to_string(), glfw_poll_events);
    map.insert("glfw_swap_buffers".to_string(), glfw_swap_buffers);
    map.insert("glfw_get_proc_address".to_string(), glfw_get_proc_address);
    map.insert("glfw_get_key".to_string(), glfw_get_key);
    map.insert("glfw_get_time".to_string(), glfw_get_time);
}

// --- IMPLEMENTATION ---

fn glfw_init(_: Vec<Value>) -> Result<Value, String> {
    let glfw = glfw::init(glfw::fail_on_errors)
        .map_err(|e| format!("GLFW Init Error: {}", e))?;

    let state = GlfwState {
        context: glfw,
        windows: HashMap::new(),
        next_id: 1,
    };

    let mut guard = STATE.lock().unwrap();
    // On emballe dans le wrapper ThreadSafeState
    *guard = Some(ThreadSafeState(state));

    println!("[Rust-GLFW] Initialized successfully.");
    Ok(Value::Boolean(true))
}

fn glfw_create_window(args: Vec<Value>) -> Result<Value, String> {
    if args.len() != 3 { return Err("Args: width, height, title".into()); }
    
    let width = args[0].as_int()? as u32;
    let height = args[1].as_int()? as u32;
    let title = args[2].as_str()?;

    let mut guard = STATE.lock().unwrap();
    // On accède au champ .0 du wrapper
    let state_wrapper = guard.as_mut().ok_or("GLFW not initialized")?;
    let state = &mut state_wrapper.0;

    let (mut window, events) = state.context.create_window(width, height, &title, glfw::WindowMode::Windowed)
        .ok_or("Failed to create GLFW window")?;

    window.set_key_polling(true);
    window.make_current();

    let id = state.next_id;
    // Les types correspondent maintenant grâce à PWindow dans la struct
    state.windows.insert(id, (window, events));
    state.next_id += 1;

    println!("[Rust-GLFW] Window created with ID: {}", id);
    Ok(Value::Integer(id as i64))
}

fn glfw_window_should_close(args: Vec<Value>) -> Result<Value, String> {
    let id = args[0].as_int()? as usize;
    let mut guard = STATE.lock().unwrap();
    let state_wrapper = guard.as_mut().ok_or("GLFW not initialized")?;
    let state = &mut state_wrapper.0;
    
    if let Some((window, _)) = state.windows.get(&id) {
        return Ok(Value::Boolean(window.should_close()));
    }
    Ok(Value::Boolean(true))
}

fn glfw_swap_buffers(args: Vec<Value>) -> Result<Value, String> {
    let id = args[0].as_int()? as usize;
    let mut guard = STATE.lock().unwrap();
    let state_wrapper = guard.as_mut().ok_or("GLFW not initialized")?;
    let state = &mut state_wrapper.0;

    if let Some((window, _)) = state.windows.get_mut(&id) {
        window.swap_buffers();
    }
    Ok(Value::Null)
}

fn glfw_poll_events(_: Vec<Value>) -> Result<Value, String> {
    let mut guard = STATE.lock().unwrap();
    let state_wrapper = guard.as_mut().ok_or("GLFW not initialized")?;
    let state = &mut state_wrapper.0;
    
    state.context.poll_events();
    Ok(Value::Null)
}

fn glfw_get_proc_address(_: Vec<Value>) -> Result<Value, String> {
    let ptr = glfw::ffi::glfwGetProcAddress as usize;
    Ok(Value::Integer(ptr as i64))
}

fn glfw_get_key(args: Vec<Value>) -> Result<Value, String> {
    if args.len() != 2 {
        return Err("Args: win_id, key_code".into());
    }

    let id = args[0].as_int()? as usize;
    let key_code = args[1].as_int()? as i32;

    let mut guard = STATE.lock().unwrap();
    let state_wrapper = guard.as_mut().ok_or("GLFW not initialized")?;
    let state = &mut state_wrapper.0;

    if let Some((window, _)) = state.windows.get(&id) {
        // Convert raw int to GLFW Key enum (unsafe but necessary for raw binding)
        // Or simpler: use glfw::Key::from_i32 if available, or just map manually.
        // For simplicity in a dynamic binding, we trust the integer passed matches GLFW constants.
        // Note: glfw-rs expects a Key enum. We need a way to cast int to Key.
        // Since we can't easily cast int to Enum in safe Rust without a huge match,
        // let's assume the user passes the correct ID.
        
        // Hack: Transmute int to Key (works because Key is repr(i32) usually)
        // A cleaner way would be a huge match statement, but for a binding engine:
        let key: glfw::Key = unsafe { std::mem::transmute(key_code) };
        
        let action = window.get_key(key);
        // Returns true if Press or Repeat
        return Ok(Value::Boolean(action == glfw::Action::Press || action == glfw::Action::Repeat));
    }

    Ok(Value::Boolean(false))
}

fn glfw_get_time(_: Vec<Value>) -> Result<Value, String> {
    let mut guard = STATE.lock().unwrap();
    let state_wrapper = guard.as_mut().ok_or("GLFW not initialized")?;
    let state = &mut state_wrapper.0;

    Ok(Value::Float(state.context.get_time()))
}
