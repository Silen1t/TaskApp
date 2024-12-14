#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use dirs::data_local_dir;
use rand::random;
use serde::{Deserialize, Serialize};
use slint::{LogicalSize, Model, ModelRc, VecModel, WindowSize};
use std::{
    error::Error,
    fs::{self, File, OpenOptions},
    io::BufReader,
    path::PathBuf,
    sync::{Arc, Mutex},
};

slint::include_modules!();

const APP_NAME: &str = "TaskApp";
const FILE_NAME: &str = "TaskApp.json";

#[derive(Serialize, Deserialize)]
struct SaveTasks {
    tasks: Vec<TaskCardInfo>,
}

#[derive(Serialize, Deserialize, Clone)]
struct TaskCardInfo {
    id: f32,
    text: String,
    check: bool,
}

impl SaveTasks {
    /// Get the path to the save file in the user's AppData directory
    fn get_save_path() -> PathBuf {
        let app_dir = data_local_dir()
            .expect("Unable to locate AppData directory")
            .join(APP_NAME);
        
        fs::create_dir_all(&app_dir).unwrap_or_else(|_| panic!("Failed to create directory: {:?}", app_dir));
        app_dir.join(FILE_NAME)
    }

    /// Load tasks from the save file or return an empty list if the file does not exist
    fn load() -> Self {
        let path = Self::get_save_path();
        if path.exists() {
            let file = File::open(path).expect("Failed to open save file");
            let reader = BufReader::new(file);
            serde_json::from_reader(reader).unwrap_or_default()
        } else {
            Self::default()
        }
    }

    /// Save the current tasks to the file
    fn save_to_file(&self) {
        let path = Self::get_save_path();
        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(path)
            .expect("Failed to open save file");
        serde_json::to_writer(file, self).expect("Failed to write to save file");
    }

    /// Add a new task to the list and save
    fn store(&mut self, task: TaskCardInfo) {
        self.tasks.push(task);
        self.save_to_file();
    }

    /// Remove a task by its ID and save
    fn remove_task(&mut self, task_id: f32) -> Result<(), &'static str> {
        if let Some(pos) = self.tasks.iter().position(|t| t.id == task_id) {
            self.tasks.remove(pos);
            self.save_to_file();
            Ok(())
        } else {
            Err("Task not found")
        }
    }
}

impl Default for SaveTasks {
    fn default() -> Self {
        Self { tasks: Vec::new() }
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let app = Arc::new(AppWindow::new()?);
    setup_window(&app);

    let loaded_tasks = Arc::new(Mutex::new(SaveTasks::load()));

    initialize_task_list(&app, &loaded_tasks);
    setup_event_handlers(&app, &loaded_tasks);

    app.run()?;
    Ok(())
}


/// Initialize the task list in the UI
fn initialize_task_list(app: &Arc<AppWindow>, loaded_tasks: &Arc<Mutex<SaveTasks>>) {
    if let Ok(tasks) = loaded_tasks.lock() {
        let task_model = VecModel::from(
            tasks.tasks.iter().map(|task| Task {
                id: task.id,
                text: task.text.clone().into(),
                check: task.check,
            }).collect::<Vec<_>>(),
        );
        app.set_tasks(ModelRc::new(task_model));
    }
}

/// Configure the window settings
fn setup_window(app: &Arc<AppWindow>) {
    let window_size = WindowSize::Logical(LogicalSize::new(800.0, 850.0));
    app.window().set_size(window_size);
}

/// Setup event handlers for all task-related actions
fn setup_event_handlers(app: &Arc<AppWindow>, loaded_tasks: &Arc<Mutex<SaveTasks>>) {
    setup_add_task_event(app, loaded_tasks);
    setup_remove_task_event(app, loaded_tasks);
    setup_check_task_event(app, loaded_tasks);
}

/// Setup the event handler for adding tasks
fn setup_add_task_event(app: &Arc<AppWindow>, loaded_tasks: &Arc<Mutex<SaveTasks>>) {
    let save_tasks = Arc::clone(loaded_tasks);
    let app_weak = app.as_weak(); // Create a Weak reference to app

    app.on_add_task(move |task_title, tasks_model| {
        if task_title.is_empty() {
            return;
        }

        let id = random::<f32>();
        let new_task = Task {
            id,
            text: task_title.clone(),
            check: false,
        };

        let mut tasks_vec: Vec<Task> = tasks_model.iter().collect();
        tasks_vec.push(new_task.clone());

        if let Some(app) = app_weak.upgrade() { // Upgrade the Weak reference
            app.set_tasks(ModelRc::new(VecModel::from(tasks_vec)));
        }

        if let Ok(mut tasks) = save_tasks.lock() {
            tasks.store(TaskCardInfo {
                id,
                text: task_title.to_string(),
                check: false,
            });
        }
    });
}

/// Setup the event handler for removing tasks
fn setup_remove_task_event(app: &Arc<AppWindow>, loaded_tasks: &Arc<Mutex<SaveTasks>>) {
    let save_tasks = Arc::clone(loaded_tasks);
    let app_weak = app.as_weak(); // Create a Weak reference to app
    
    app.on_remove_task(move |task, _| {
        // Attempt to upgrade the Weak reference to an Arc
        if let Some(app) = app_weak.upgrade() {
            if let Ok(mut tasks) = save_tasks.lock() {
                if tasks.remove_task(task.id).is_err() {
                    eprintln!("Failed to remove task with ID: {}", task.id);
                }
            }

            let updated_model = remove_task_from_model(&app.get_tasks(), &task);
            app.set_tasks(updated_model);
        } else {
            eprintln!("App reference is no longer valid");
        }
    });
}

/// Setup the event handler for checking/unchecking tasks
fn setup_check_task_event(app: &Arc<AppWindow>, loaded_tasks: &Arc<Mutex<SaveTasks>>) {
    let save_tasks = Arc::clone(loaded_tasks);
    let app_weak = app.as_weak(); // Create a Weak reference to app

    app.on_check_task(move |task, tasks_model| {
        // Attempt to upgrade the Weak reference to an Arc
        if let Some(app) = app_weak.upgrade() {
            if let Ok(mut tasks) = save_tasks.lock() {
                if let Some(t) = tasks.tasks.iter_mut().find(|t| t.id == task.id) {
                    t.check = task.check;
                    tasks.save_to_file();
                }
            }

            let mut task_list: Vec<Task> = tasks_model.iter().collect();
            if let Some(t) = task_list.iter_mut().find(|t| t.id == task.id) {
                t.check = task.check;
            }

            app.set_tasks(ModelRc::new(VecModel::from(task_list)));
        } else {
            eprintln!("App reference is no longer valid");
        }
    });
}

/// Remove a task from the UI model
fn remove_task_from_model(model: &ModelRc<Task>, task: &Task) -> ModelRc<Task> {
    let tasks: Vec<Task> = model.iter().filter(|t| t.id != task.id).collect();
    ModelRc::new(VecModel::from(tasks))
}
