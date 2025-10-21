use clap::Parser;

use iced::event::{self, Event};
use iced::widget::{self, column};
use iced::{Element, Subscription, Task};
use iced_aw::Tabs;
use image::ImageReader;
use log::debug;

rust_i18n::i18n!("locales");

mod actions;
mod image_widget;
mod pathlist;
mod settings;
mod sorting;
mod task_manager;

use image_widget::PixelCanvasMessage;
use pathlist::PathList;

use settings::{SettingsMessage, SettingsModel};
use sorting::{SortingMessage, Tag, TagNames};
use task_manager::{TaskId, TaskManager, TaskType};

use crate::sorting::Dim;
use crate::task_manager::TaskCompleteResult;

const PICTURE_DIR: &str = ".";
pub const PRELOAD_IN_FLIGHT: usize = 8;
#[allow(dead_code)]
const PRELOAD_CACHE_SIZE: usize = 100;

#[derive(Parser)]
struct Args {
    #[arg(default_value = ".")]
    input_dir: String,
}

pub fn main() -> iced::Result {
    simplelog::CombinedLogger::init(vec![
        simplelog::TermLogger::new(
            simplelog::LevelFilter::Debug,
            simplelog::ConfigBuilder::new()
                .add_filter_allow_str("imgsort")
                .build(),
            simplelog::TerminalMode::Mixed,
            simplelog::ColorChoice::Auto,
        ),
        simplelog::WriteLogger::new(
            simplelog::LevelFilter::Debug,
            simplelog::ConfigBuilder::new()
                .add_filter_allow_str("imgsort")
                .build(),
            std::fs::File::create("imgsort.log").unwrap(),
        ),
    ])
    .unwrap();

    rust_i18n::set_locale("se");

    let args = Args::parse();

    if std::env::set_current_dir(&args.input_dir).is_err() {
        println!("Error opening directory {}", args.input_dir);
        std::process::exit(1);
    }

    iced::application(Model::title, Model::update_with_task, Model::view)
        .subscription(Model::subscription)
        .run_with(Model::new_with_task)
}

#[derive(Debug)]
struct Model {
    config: Config,
    state: ModelState,
    settings: SettingsModel,
    active_tab: TabId,
    selected_action_tag: Option<Tag>,
    task_manager: TaskManager,
    pathlist: PathList,
    expanded_dropdown: Option<Tag>,
    editing_tag_name: Option<(Tag, String, widget::text_input::Id)>,
    tag_names: TagNames,
    canvas_dimensions: Option<Dim>,
}

#[derive(Debug)]
enum ModelState {
    LoadingListDir,
    EmptyDirectory,
    Sorting,
}

#[derive(Debug, Clone)]
pub struct Config {
    preload_back_num: usize,
    preload_front_num: usize,
    scale_down_size: (u32, u32),
    thumbnail_size: Dim,
    thumbnail_style: SortingViewStyle,
}

#[derive(Debug)]
pub struct ImageInfo {
    pub path: String,
    pub data: PreloadImage,
    pub metadata: Metadata,
}

#[derive(Debug)]
pub struct Metadata {
    pub tag: Option<Tag>,
}

#[derive(Clone)]
pub struct ImageData {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
}

#[derive(PartialEq, Eq, Debug, Clone, Copy)]
pub enum TabId {
    Main,
    Actions,
    Settings,
}

impl std::fmt::Debug for ImageData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ImageData")
            .field("width", &self.width)
            .field("height", &self.height)
            .field("data", &format_args!("{} bytes", self.data.len()))
            .finish()
    }
}

#[derive(Debug, Clone)]
pub enum Message {
    UserPressedSelectFolder,
    UserSelectedTab(TabId),
    UserPressedActionTag(Tag),
    UserPressedActionBack,
    UserPressedActionCopy(Tag),
    ListDirCompleted(TaskId, Vec<String>),
    ImagePreloaded(TaskId, String, ImageData, ImageData),
    KeyboardEventOccurred(iced::keyboard::Event),
    Settings(SettingsMessage),
    Sorting(SortingMessage),
    PixelCanvas(PixelCanvasMessage),
}

#[derive(Debug)]
pub enum PreloadImage {
    Loading(String),
    Loaded(LoadedImageAndThumb),
    NotLoading,
}

#[derive(Debug)]
pub struct LoadedImageAndThumb {
    pub image: ImageData,
    pub thumb: ImageData,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Effect {
    None,
    LsDir,
    PreloadImages(Vec<String>, Dim),
    MoveThenLs(Tag),
    FocusElement(widget::text_input::Id),
}

impl Model {
    fn new() -> (Self, Effect) {
        let config = Config {
            preload_back_num: 10,
            preload_front_num: 30,
            scale_down_size: (800, 100),
            thumbnail_size: Dim {
                width: 100,
                height: 100,
            },
            thumbnail_style: SortingViewStyle::ThumbsAbove,
        };
        (
            Self {
                config: config.clone(),
                state: ModelState::LoadingListDir,
                settings: SettingsModel::new(&config),
                active_tab: TabId::Main,
                selected_action_tag: None,
                task_manager: TaskManager::new(),
                pathlist: PathList::new(vec![]),
                expanded_dropdown: None,
                editing_tag_name: None,
                tag_names: TagNames::new(),
                canvas_dimensions: None,
            },
            Effect::LsDir,
        )
    }

    fn new_with_task() -> (Self, Task<Message>) {
        let (mut new_self, effect) = Self::new();
        let task = effect_to_task(effect, &mut new_self);
        (new_self, task)
    }

    fn subscription(&self) -> Subscription<Message> {
        event::listen_with(Self::subscription_keyboard_filter).map(Message::KeyboardEventOccurred)
    }

    fn subscription_keyboard_filter(
        event: Event,
        _status: event::Status,
        _id: iced::window::Id,
    ) -> Option<iced::keyboard::Event> {
        match event {
            Event::Keyboard(keyboard_event) => Some(keyboard_event),
            _ => None,
        }
    }

    fn go_to_sorting_model(&mut self, paths: Vec<String>) -> Effect {
        match self.state {
            ModelState::Sorting => {
                debug!("In sorting model, received new lsdir, updating");

                // Pathlist
                let index: usize = {
                    if let Some(previous_image) = self
                        .pathlist
                        .paths
                        .get(self.pathlist.index)
                        .map(|info| &info.path)
                    {
                        paths.iter().position(|p| p == previous_image).unwrap_or(0)
                    } else {
                        0
                    }
                };

                // TODO, use previous image data here instead of clearing
                let paths = paths
                    .iter()
                    .map(|path| ImageInfo {
                        path: path.clone(),
                        data: PreloadImage::NotLoading,
                        metadata: Metadata {
                            tag: self.pathlist.tag_of(path),
                        },
                    })
                    .collect();

                self.pathlist = PathList { index, paths };
            }

            _ => {
                debug!("Going to new sorting model");

                self.state = ModelState::Sorting;
                self.pathlist = PathList::new(paths.clone());
                self.expanded_dropdown = None;
                self.editing_tag_name = None;
                self.tag_names = TagNames::new();
                self.canvas_dimensions = None;
            }
        };
        let preload_images = self.pathlist.get_initial_preload_images(&self.config);

        if let Some(dimensions) = self.canvas_dimensions {
            Effect::PreloadImages(preload_images, dimensions)
        } else {
            Effect::None
        }
    }

    fn title(&self) -> String {
        "ImageViewer".to_owned()
    }

    fn update_with_task(&mut self, message: Message) -> Task<Message> {
        let effect = self.update(message);

        effect_to_task(effect, self)
    }

    fn update(&mut self, message: Message) -> Effect {
        debug!("Message: {message:?}");
        let effect = match message {
            Message::UserPressedActionCopy(tag) => Effect::MoveThenLs(tag),
            Message::UserSelectedTab(tab) => {
                self.active_tab = tab;
                self.selected_action_tag = None;
                Effect::None
            }
            Message::UserPressedActionTag(tag) => {
                self.selected_action_tag = Some(tag);
                Effect::None
            }
            Message::UserPressedActionBack => {
                self.selected_action_tag = None;
                Effect::None
            }
            Message::UserPressedSelectFolder => Effect::None,
            Message::ListDirCompleted(task_id, paths) => {
                if self.task_manager.report_completed_task(task_id)
                    == TaskCompleteResult::TaskWasCancelled
                {
                    return Effect::None;
                };
                self.task_manager.cancel_all();
                debug!("Directory listing completed for task {:?}", task_id);
                if paths.is_empty() {
                    self.state = ModelState::EmptyDirectory;
                    Effect::None
                } else {
                    self.go_to_sorting_model(paths)
                }
            }
            Message::ImagePreloaded(task_id, path, image, thumb) => {
                self.task_manager.report_completed_task(task_id);
                debug!("Image preload completed for task {:?}", task_id);
                match self.state {
                    ModelState::Sorting => {
                        self.update_sorting(SortingMessage::ImagePreloaded(path, image, thumb))
                    }
                    _ => Effect::None,
                }
            }
            Message::KeyboardEventOccurred(event) => match self.state {
                ModelState::Sorting => self.update_sorting(SortingMessage::KeyboardEvent(event)),
                _ => Effect::None,
            },
            Message::Sorting(sorting_message) => match self.state {
                ModelState::Sorting => self.update_sorting(sorting_message),
                _ => Effect::None,
            },
            Message::Settings(settings_message) => {
                self.settings.update(settings_message, &mut self.config)
            }
            Message::PixelCanvas(pixel_canvas_message) => match self.state {
                ModelState::Sorting => match pixel_canvas_message {
                    PixelCanvasMessage::CanvasSized(dim) => {
                        self.update_sorting(SortingMessage::CanvasResized(dim))
                    }
                },
                _ => Effect::None,
            },
        };

        debug!("Effect: {effect:?}");
        effect
    }

    fn view(&self) -> Element<Message> {
        let main_content = match self.state {
            ModelState::Sorting => self.view_sorting(),
            ModelState::LoadingListDir => {
                let loading_text = if self.task_manager.is_loading() {
                    self.task_manager.get_loading_text()
                } else {
                    "Loading...".to_string()
                };
                widget::text(loading_text).into()
            }
            ModelState::EmptyDirectory => self.view_empty_dir_model(),
        };

        let tag_names = match self.state {
            ModelState::Sorting => self.tag_names.clone(),
            _ => TagNames::new(),
        };
        let actions_content = actions::view_actions_tab(&self.selected_action_tag, &tag_names);

        let settings_content = self.settings.view();

        Tabs::new(Message::UserSelectedTab)
            .push(
                TabId::Main,
                iced_aw::TabLabel::Text(String::from("Main")),
                main_content,
            )
            .push(
                TabId::Actions,
                iced_aw::TabLabel::Text(String::from("Actions")),
                actions_content,
            )
            .push(
                TabId::Settings,
                iced_aw::TabLabel::Text(String::from("Settings")),
                settings_content,
            )
            .set_active_tab(&self.active_tab)
            .into()
    }

    fn view_empty_dir_model(&self) -> Element<'static, Message> {
        column![
            widget::text("No pictures in this directory, select another one"),
            widget::button("Select Folder").on_press(Message::UserPressedSelectFolder),
        ]
        .into()
    }
}

impl Model {
    fn update_sorting(&mut self, message: SortingMessage) -> Effect {
        let config = self.config.clone();
        sorting::update_sorting_model(self, message, &config)
    }

    fn view_sorting(&self) -> iced::Element<'_, Message> {
        sorting::view_sorting_model(self, &self.config, &self.task_manager)
    }
}

fn effect_to_task(effect: Effect, model: &mut Model) -> Task<Message> {
    match effect {
        Effect::None => Task::none(),
        Effect::LsDir => {
            model.task_manager.cancel_all();

            model.task_manager.start_task(
                TaskType::LsDir,
                Message::ListDirCompleted,
                get_files_in_folder_async(PICTURE_DIR.to_owned()),
            )
        }
        Effect::PreloadImages(paths, dim) => {
            preload_images_task(paths, dim, model.config.clone(), &mut model.task_manager)
        }
        Effect::MoveThenLs(tag) => {
            let files_to_move = model
                .pathlist
                .paths
                .iter()
                .filter_map(|info| {
                    if info.metadata.tag == Some(tag) {
                        Some(info.path.clone())
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>();
            let tag_name = model.tag_names.get(&tag);
            if files_to_move.is_empty() {
                println!("No files to move");
                Task::none()
            } else {
                println!("mv {} \"{}\"", files_to_move.join(" "), tag_name);

                model.task_manager.start_task(
                    TaskType::MoveThenLs,
                    Message::ListDirCompleted,
                    mv_then_ls_async(files_to_move, tag_name.to_string()),
                )
            }
        }
        Effect::FocusElement(id) => widget::text_input::focus(id),
    }
}

async fn mv_then_ls_async(files: Vec<String>, destination: String) -> Vec<String> {
    match tokio::task::spawn_blocking(move || {
        mv_files(files, destination);
        get_files_in_folder(PICTURE_DIR)
    })
    .await
    .expect("Could not spawn task")
    {
        Ok(files_in_folder) => files_in_folder,
        Err(_) => panic!("Io Error when listing directory after move"),
    }
}

fn mv_files(files: Vec<String>, destination: String) {
    // Create directory if it doesn't exist
    let dest_path = std::path::Path::new(&destination);
    if !dest_path.exists() {
        std::fs::create_dir(dest_path).unwrap();
    }
    let dest_path = std::path::Path::new(&destination).canonicalize().unwrap();
    for file in files {
        println!("Moving {file} to {destination}");
        let basename = std::path::Path::new(&file).file_name().unwrap();
        let mut dest = dest_path.clone();
        dest.push(basename);
        std::fs::rename(&file, dest).unwrap();
    }
}

async fn get_files_in_folder_async(folder_path: String) -> Vec<String> {
    match tokio::task::spawn_blocking(move || get_files_in_folder(folder_path.as_str())).await {
        Ok(Ok(res)) => res,
        Ok(Err(_)) => panic!("Io Error when listing directory after move"),
        Err(_) => panic!("Could not spawn task"),
    }
}

fn get_files_in_folder(folder_path: &str) -> std::io::Result<Vec<String>> {
    let mut file_names = Vec::new();
    let entries = std::fs::read_dir(folder_path)?;

    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() {
            if let Some(file_name) = path.file_name() {
                if let Some(file_name_str) = file_name.to_str() {
                    if file_name_str.ends_with(".jpg") || file_name_str.ends_with(".png") {
                        file_names.push(format!("{folder_path}/{file_name_str}"));
                    }
                }
            }
        }
    }

    file_names.sort();
    Ok(file_names)
}

fn preload_images_task(
    paths: Vec<String>,
    dim: Dim,
    config: Config,
    task_manager: &mut TaskManager,
) -> Task<Message> {
    let mut tasks = Vec::new();
    for path in paths {
        let config2 = config.clone();

        let task = task_manager.start_task(
            TaskType::PreloadImage,
            |task_id, (a, b, c)| Message::ImagePreloaded(task_id, a, b, c),
            preload_image_async(path, dim, config2),
        );

        tasks.push(task);
    }
    Task::batch(tasks)
}

async fn preload_image_async(
    path: String,
    dim: Dim,
    config: Config,
) -> (String, ImageData, ImageData) {
    tokio::task::spawn_blocking(move || preload_image(path, dim, config))
        .await
        .expect("Could not spawn task")
}

fn preload_image(path: String, dim: Dim, config: Config) -> (String, ImageData, ImageData) {
    let image = get_resized_image(&path, dim);
    let thumb = get_resized_image(&path, config.thumbnail_size);
    (path, image, thumb)
}

fn get_resized_image(path: &str, dim: Dim) -> ImageData {
    let image = ImageReader::open(path)
        .unwrap()
        .decode()
        .unwrap()
        .resize(dim.width, dim.height, image::imageops::FilterType::Triangle)
        .to_rgba8();
    let width = image.width();
    let height = image.height();
    let image = ImageData {
        data: image.to_vec(),
        width,
        height,
    };
    image
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SortingViewStyle {
    NoThumbnails,
    ThumbsAbove,
}

impl SortingViewStyle {
    pub fn display_name(&self) -> &'static str {
        match self {
            SortingViewStyle::NoThumbnails => "No Thumbnails",
            SortingViewStyle::ThumbsAbove => "Thumbnails Above",
        }
    }

    pub fn all_variants() -> Vec<SortingViewStyle> {
        vec![
            SortingViewStyle::NoThumbnails,
            SortingViewStyle::ThumbsAbove,
        ]
    }

    pub fn from_display_name(name: &str) -> Option<SortingViewStyle> {
        match name {
            "No Thumbnails" => Some(SortingViewStyle::NoThumbnails),
            "Thumbnails Above" => Some(SortingViewStyle::ThumbsAbove),
            _ => None,
        }
    }
}

impl std::fmt::Display for SortingViewStyle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display_name())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
}
