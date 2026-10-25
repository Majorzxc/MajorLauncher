//! Панель «Загрузки»: что лаунчер сейчас качает или проверяет, куда и сколько
//! осталось. Задачи живут, пока открыт лаунчер; окно получает весь список
//! событием `tasks` (во время загрузки — не чаще раза в 150 мс).

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use majorlauncher_core::Event;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::AppState;

/// Сколько завершённых задач помнить.
const KEEP_FINISHED: usize = 30;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum TaskState {
    Running,
    Done,
    Failed,
    /// Игрок нажал «Отменить».
    Cancelled,
}

#[derive(Debug, Clone, Serialize)]
pub struct Task {
    pub id: u64,
    /// Что делается: «Sodium, Iris», «Файлы игры 1.21.1».
    pub title: String,
    /// Куда: «в сборку «Выживание»», «в библиотеку».
    pub place: String,
    /// Скачано и всего, байт; `total = 0` — размер ещё неизвестен.
    pub done: u64,
    pub total: u64,
    pub files_done: usize,
    pub files_total: usize,
    /// Что происходит сейчас, если не загрузка: «Проверка файлов», «Ставим загрузчик 3/6».
    pub stage: Option<String>,
    pub state: TaskState,
    pub error: Option<String>,
    pub started: u64,
    pub finished: Option<u64>,
}

#[derive(Default)]
pub struct Tasks {
    next: u64,
    list: Vec<Task>,
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

fn emit(app: &AppHandle) {
    let list = app.state::<AppState>().tasks.lock().unwrap().list.clone();
    let _ = app.emit("tasks", list);
}

/// Задача в работе. Закрывается `finish`; брошенная без него — считается
/// оборванной.
pub struct TaskHandle {
    app: AppHandle,
    id: u64,
    last: Mutex<Instant>,
    closed: Mutex<bool>,
}

impl TaskHandle {
    fn update(&self, f: impl FnOnce(&mut Task), force: bool) {
        {
            let state = self.app.state::<AppState>();
            let mut tasks = state.tasks.lock().unwrap();
            if let Some(t) = tasks.list.iter_mut().find(|t| t.id == self.id) {
                f(t);
            }
        }
        let mut last = self.last.lock().unwrap();
        if force || last.elapsed() >= Duration::from_millis(150) {
            *last = Instant::now();
            emit(&self.app);
        }
    }

    /// Событие загрузки из ядра.
    pub fn event(&self, event: &Event) {
        match *event {
            Event::Downloading { files, bytes } => self.update(
                |t| {
                    t.files_total += files;
                    t.total += bytes;
                    t.stage = None;
                },
                true,
            ),
            Event::FileDone { bytes } => self.update(
                |t| {
                    t.files_done += 1;
                    t.done += bytes;
                },
                false,
            ),
            Event::Loader { step, total } => {
                self.stage(format!("Ставим загрузчик {step}/{total}"));
            }
            Event::Checking { .. } | Event::Extracting => {}
        }
    }

    pub fn stage(&self, text: impl Into<String>) {
        let text = text.into();
        self.update(|t| t.stage = Some(text), true);
    }

    /// Обработчик событий ядра для этой задачи.
    pub fn progress(self: &Arc<Self>) -> impl Fn(Event) + Send + Sync + 'static {
        let me = self.clone();
        move |e| me.event(&e)
    }

    pub fn finish<T, E: std::fmt::Display>(&self, result: &Result<T, E>) {
        *self.closed.lock().unwrap() = true;
        let error = result.as_ref().err().map(|e| e.to_string());
        let cancelled =
            error.as_deref() == Some(&*majorlauncher_core::Error::Cancelled.to_string());
        self.update(
            |t| {
                t.state = match &error {
                    _ if cancelled => TaskState::Cancelled,
                    Some(_) => TaskState::Failed,
                    None => TaskState::Done,
                };
                t.error = if cancelled { None } else { error };
                t.stage = None;
                t.finished = Some(now());
            },
            true,
        );
    }
}

impl Drop for TaskHandle {
    fn drop(&mut self) {
        if !*self.closed.lock().unwrap() {
            self.finish::<(), _>(&Err("прервано"));
        }
    }
}

/// Начать задачу и показать её в панели.
pub fn start(
    app: &AppHandle,
    title: impl Into<String>,
    place: impl Into<String>,
) -> Arc<TaskHandle> {
    let id = {
        let state = app.state::<AppState>();
        let mut tasks = state.tasks.lock().unwrap();
        tasks.next += 1;
        let id = tasks.next;
        tasks.list.push(Task {
            id,
            title: title.into(),
            place: place.into(),
            done: 0,
            total: 0,
            files_done: 0,
            files_total: 0,
            stage: None,
            state: TaskState::Running,
            error: None,
            started: now(),
            finished: None,
        });
        // Старые завершённые — вон, чтобы список не рос.
        let finished = tasks
            .list
            .iter()
            .filter(|t| t.state != TaskState::Running)
            .count();
        if finished > KEEP_FINISHED {
            let mut drop = finished - KEEP_FINISHED;
            tasks.list.retain(|t| {
                if drop > 0 && t.state != TaskState::Running {
                    drop -= 1;
                    false
                } else {
                    true
                }
            });
        }
        id
    };
    emit(app);
    Arc::new(TaskHandle {
        app: app.clone(),
        id,
        last: Mutex::new(Instant::now() - Duration::from_secs(1)),
        closed: Mutex::new(false),
    })
}

#[tauri::command]
pub fn tasks_list(state: State<AppState>) -> Vec<Task> {
    state.tasks.lock().unwrap().list.clone()
}

/// Убрать из панели завершённые задачи.
#[tauri::command]
pub fn tasks_clear(app: AppHandle, state: State<AppState>) {
    state
        .tasks
        .lock()
        .unwrap()
        .list
        .retain(|t| t.state == TaskState::Running);
    emit(&app);
}

/// Задача, которая появляется, только если есть что качать: запуск игры с
/// уже скачанными файлами в панель не попадает.
pub struct LazyTask {
    app: AppHandle,
    title: String,
    place: String,
    slot: Mutex<Option<Arc<TaskHandle>>>,
}

impl LazyTask {
    pub fn new(app: &AppHandle, title: impl Into<String>, place: impl Into<String>) -> Arc<Self> {
        Arc::new(Self {
            app: app.clone(),
            title: title.into(),
            place: place.into(),
            slot: Mutex::new(None),
        })
    }

    /// Событие ядра: загрузка или установка загрузчика открывают задачу.
    pub fn event(&self, event: &Event) {
        let mut slot = self.slot.lock().unwrap();
        if slot.is_none() && matches!(event, Event::Downloading { .. } | Event::Loader { .. }) {
            *slot = Some(start(&self.app, self.title.clone(), self.place.clone()));
        }
        if let Some(task) = slot.as_ref() {
            task.event(event);
        }
    }

    pub fn finish<T, E: std::fmt::Display>(&self, result: &Result<T, E>) {
        if let Some(task) = self.slot.lock().unwrap().take() {
            task.finish(result);
        }
    }
}
