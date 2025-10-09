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
mod settings;
mod sorting;

use image_widget::PixelCanvasMessage;

use settings::{SettingsMessage, SettingsModel};
use sorting::{SortingMessage, SortingModel, Tag, TagNames};

const PICTURE_DIR: &str = ".";
const PRELOAD_IN_FLIGHT: usize = 8;
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
            simplelog::Config::default(),
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
}

#[derive(Debug)]
enum ModelState {
    LoadingListDir,
    EmptyDirectory,
    Sorting(SortingModel),
}

#[derive(Debug, Clone)]
struct Config {
    preload_back_num: usize,
    preload_front_num: usize,
    scale_down_size: (u32, u32),
}

#[derive(Debug)]
struct ImageInfo {
    path: String,
    data: PreloadImage,
    metadata: Metadata,
}

#[derive(Debug)]
struct Metadata {
    tag: Option<Tag>,
}

#[derive(Clone)]
struct ImageData {
    width: u32,
    height: u32,
    data: Vec<u8>,
}

#[derive(PartialEq, Eq, Debug, Clone, Copy)]
enum TabId {
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
enum Message {
    UserPressedSelectFolder,
    UserSelectedTab(TabId),
    UserPressedActionTag(Tag),
    UserPressedActionBack,
    ListDirCompleted(Vec<String>),
    KeyboardEventOccurred(iced::keyboard::Event),
    Settings(SettingsMessage),
    Sorting(SortingMessage),
    PixelCanvas(PixelCanvasMessage),
}

#[derive(Debug)]
struct PathList {
    paths: Vec<ImageInfo>,
    index: usize,
    preload_back_num: usize,
    preload_front_num: usize,
}

impl PathList {
    fn new(paths: Vec<String>, preload_back_num: usize, preload_front_num: usize) -> Self {
        let paths = paths
            .iter()
            .map(|path| ImageInfo {
                path: path.clone(),
                data: PreloadImage::OutOfRange,
                metadata: Metadata { tag: None },
            })
            .collect();
        Self {
            paths,
            index: 0,
            preload_back_num,
            preload_front_num,
        }
    }

    // Preload order?
    // cache-size = 100, how many picture are kept in the list, when you scroll past preload limit
    // back = 10, how many you start preloading backwards
    // front = 30, how many you start preloading forwards
    // in_flight = 8 (Or number of cores?), how many you preload at the same time
    fn get_initial_preload_images(&self) -> Vec<String> {
        let mut paths = Vec::new();
        let from = self
            .index
            .saturating_sub(std::cmp::min(self.preload_back_num, PRELOAD_IN_FLIGHT / 2));
        let to = *[
            self.index + self.preload_front_num + 1,
            self.paths.len(),
            from + PRELOAD_IN_FLIGHT,
        ]
        .iter()
        .min()
        .expect("The iter is not empty");

        for i in from..to {
            paths.push(self.paths[i].path.clone());
        }
        paths
    }

    fn tag_of(&self, path: &str) -> Option<Tag> {
        self.paths
            .iter()
            .find(|info| info.path == path)
            .and_then(|info| info.metadata.tag)
    }
    fn prev(&self) -> Option<&ImageInfo> {
        if self.index == 0 {
            None
        } else {
            Some(&self.paths[self.index - 1])
        }
    }

    fn current(&self) -> &ImageInfo {
        &self.paths[self.index]
    }

    fn next(&self) -> Option<&ImageInfo> {
        self.paths.get(self.index + 1)
    }

    fn current_mut(&mut self) -> &mut ImageInfo {
        &mut self.paths[self.index]
    }
}

#[derive(Debug)]
enum PreloadImage {
    Loading(String),
    Loaded(ImageData),
    OutOfRange,
}

#[derive(Debug, Clone, Eq, PartialEq)]
enum Effect {
    None,
    LsDir,
    PreloadImages(Vec<String>),
    MoveImagesWithTag(Tag),
    FocusElement(widget::text_input::Id),
}

impl Model {
    fn new() -> (Self, Effect) {
        let config = Config {
            preload_back_num: 10,
            preload_front_num: 30,
            scale_down_size: (800, 100),
        };
        (
            Self {
                config: config.clone(),
                state: ModelState::LoadingListDir,
                settings: SettingsModel::new(&config),
                active_tab: TabId::Main,
                selected_action_tag: None,
            },
            Effect::LsDir,
        )
    }

    fn new_with_task() -> (Self, Task<Message>) {
        let (new_self, effect) = Self::new();
        let task = effect_to_task(effect, &new_self, new_self.config.clone());
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
        match &mut self.state {
            ModelState::Sorting(sorting) => {
                debug!("In sorting model, received new lsdir, updating");

                // Pathlist
                let index: usize = {
                    if let Some(previous_image) = sorting
                        .pathlist
                        .paths
                        .get(sorting.pathlist.index)
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
                        data: PreloadImage::OutOfRange,
                        metadata: Metadata {
                            tag: sorting.pathlist.tag_of(path),
                        },
                    })
                    .collect();

                sorting.pathlist = PathList {
                    index,
                    paths,
                    preload_back_num: sorting.pathlist.preload_back_num,
                    preload_front_num: sorting.pathlist.preload_front_num,
                };
            }

            _ => {
                debug!("Going to new sorting model");

                self.state = ModelState::Sorting(SortingModel {
                    pathlist: PathList::new(
                        paths.clone(),
                        self.config.preload_back_num,
                        self.config.preload_front_num,
                    ),
                    expanded_dropdown: None,
                    editing_tag_name: None,
                    tag_names: TagNames::new(),
                    canvas_dimensions: None,
                });
            }
        };
        let ModelState::Sorting(sorting_model) = &self.state else {
            panic!()
        };
        let preload_images = sorting_model.pathlist.get_initial_preload_images();

        Effect::PreloadImages(preload_images)
    }

    fn title(&self) -> String {
        "ImageViewer".to_owned()
    }

    fn update_with_task(&mut self, message: Message) -> Task<Message> {
        effect_to_task(self.update(message), self, self.config.clone())
    }

    fn update(&mut self, message: Message) -> Effect {
        debug!("Message: {message:?}");
        let effect = match message {
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
            Message::ListDirCompleted(paths) => {
                if paths.is_empty() {
                    self.state = ModelState::EmptyDirectory;
                    Effect::None
                } else {
                    self.go_to_sorting_model(paths)
                }
            }
            Message::KeyboardEventOccurred(event) => match &mut self.state {
                ModelState::Sorting(model) => {
                    model.update(SortingMessage::KeyboardEvent(event), &self.config)
                }
                _ => Effect::None,
            },
            Message::Sorting(sorting_message) => match &mut self.state {
                ModelState::Sorting(model) => model.update(sorting_message, &self.config),
                _ => Effect::None,
            },
            Message::Settings(settings_message) => {
                self.settings.update(settings_message, &mut self.config)
            }
            Message::PixelCanvas(pixel_canvas_message) => match &mut self.state {
                ModelState::Sorting(model) => match pixel_canvas_message {
                    PixelCanvasMessage::CanvasSized(dim) => {
                        model.update(SortingMessage::CanvasResized(dim), &self.config)
                    }
                },
                _ => Effect::None,
            },
        };

        debug!("Effect: {effect:?}");
        effect
    }

    fn view(&self) -> Element<Message> {
        let main_content = match &self.state {
            ModelState::Sorting(model) => model.view(&self.config),
            ModelState::LoadingListDir => widget::text("Loading...").into(),
            ModelState::EmptyDirectory => self.view_empty_dir_model(),
        };

        let tag_names = match &self.state {
            ModelState::Sorting(model) => model.tag_names.clone(),
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
            button("Select Folder").on_press(Message::UserPressedSelectFolder),
        ]
        .into()
    }
}

impl Model {}

fn effect_to_task(effect: Effect, model: &Model, config: Config) -> Task<Message> {
    match effect {
        Effect::None => Task::none(),
        Effect::LsDir => ls_dir_task(PICTURE_DIR.to_owned()),
        Effect::PreloadImages(paths) => preload_images_task(paths, config),
        Effect::MoveImagesWithTag(tag) => {
            let (files_to_move, tag_name) = {
                let mut files_to_move = Vec::new();
                let tag_name = match &model.state {
                    ModelState::Sorting(sorting) => {
                        for info in &sorting.pathlist.paths {
                            if info.metadata.tag == Some(tag) {
                                files_to_move.push(info.path.clone());
                            }
                        }
                        sorting.tag_names.get(&tag)
                    }
                    _ => panic!("MoveImages effect should only be called in the sorting state"),
                };
                (files_to_move, tag_name)
            };
            if files_to_move.is_empty() {
                println!("No files to move");
                Task::none()
            } else {
                println!("mv {} \"{}\"", files_to_move.join(" "), tag_name);
                mv_files_task(files_to_move, tag_name.to_string())
                    .then(|()| ls_dir_task(PICTURE_DIR.to_owned()))
            }
        }
        Effect::FocusElement(id) => widget::text_input::focus(id),
    }
}

fn mv_files_task(files: Vec<String>, destination: String) -> Task<()> {
    Task::future(mv_files_async(files, destination))
}

async fn mv_files_async(files: Vec<String>, destination: String) {
    match tokio::task::spawn_blocking(move || mv_files(files, destination)).await {
        Ok(_) => (),
        Err(_) => panic!("Could not spawn task"),
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

fn ls_dir_task(path: String) -> Task<Message> {
    Task::perform(get_files_in_folder_async(path), |res| match res {
        Ok(paths) => Message::ListDirCompleted(paths),
        Err(_) => panic!("Could not list directory"),
    })
}

async fn get_files_in_folder_async(folder_path: String) -> std::io::Result<Vec<String>> {
    match tokio::task::spawn_blocking(move || get_files_in_folder(folder_path.as_str())).await {
        Ok(res) => res,
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

fn preload_images_task(paths: Vec<String>, config: Config) -> Task<Message> {
    let mut tasks = Vec::new();
    for path in paths {
        let config2 = config.clone();
        let fut = tokio::task::spawn_blocking(move || preload_image(path, config2));
        tasks.push(Task::perform(fut, |res| match res {
            Ok((path4, image)) => Message::Sorting(SortingMessage::ImagePreloaded(path4, image)),
            Err(_) => Message::Sorting(SortingMessage::ImagePreloadFailed(
                "too hard to know".to_owned(),
            )),
        }))
    }
    Task::batch(tasks)
}

fn preload_image(path: String, config: Config) -> (String, ImageData) {
    let image = ImageReader::open(path.as_str())
        .unwrap()
        .decode()
        .unwrap()
        .resize(
            config.scale_down_size.0,
            config.scale_down_size.1,
            image::imageops::FilterType::Triangle,
        )
        .to_rgba8();
    let width = image.width();
    let height = image.height();
    let image = ImageData {
        data: image.to_vec(),
        width,
        height,
    };
    (path, image)
}

fn button(text: &str) -> widget::Button<'_, Message> {
    widget::button(text).padding(10)
}

#[cfg(test)]
mod tests {}
