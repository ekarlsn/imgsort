use clap::Parser;
use itertools::Itertools;
use rust_i18n::t;
use std::collections::HashMap;

use iced::event::{self, Event};
use iced::widget::{self, center, column, row, stack};
use iced::{Color, Element, Length, Subscription, Task};
use iced_aw::{drop_down, DropDown};
use image::ImageReader;
use log::debug;

rust_i18n::i18n!("locales");

const TAGGING_CHARS: &str = "aoeupy";
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
}

#[derive(Debug)]
enum ModelState {
    LoadingListDir,
    EmptyDirectory,
    Sorting(SortingModel),
    Settings(SettingsModel),
}

#[derive(Debug)]
struct SortingModel {
    pathlist: PathList,

    // Tags
    expanded_dropdown: Option<Tag>,
    editing_tag_name: Option<(Tag, String, widget::text_input::Id)>,
    tag_names: HashMap<Tag, String>,
}

#[derive(Debug)]
struct SettingsModel {
    fields: HashMap<SettingsFieldName, (String, String)>,
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
struct PathList {
    paths: Vec<ImageInfo>,
    index: usize,
    preload_back_num: usize,
    preload_front_num: usize,
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
    UserPressedGoToSettings,
    UserPressedGoToSorting,
    UserPressedSelectFolder,
    ListDirCompleted(Vec<String>),
    KeyboardEventOccurred(iced::keyboard::Event),
    Settings(SettingsMessage),
    Sorting(SortingMessage),
}

#[derive(Debug, Clone)]
enum SortingMessage {
    UserPressedNextImage,
    UserPressedPreviousImage,
    UserPressedMoveTag(Tag),
    UserPressedTagButton(Tag),
    UserPressedRenameTag(Tag),
    UserPressedSubmitRenameTag,
    UserPressedCancelRenameTag,
    UserEditTagName(String),
    UserPressedTagMenu(Option<Tag>),
    ImagePreloaded(String, ImageData),
    ImagePreloadFailed(String),
    KeyboardEvent(iced::keyboard::Event),
}

#[derive(Debug, Clone)]
enum SettingsMessage {
    UserUpdatedField(SettingsFieldName, String),
    UserPressedBackToSorting,
    Save,
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
enum SettingsFieldName {
    PreloadBackNum,
    PreloadFrontNum,
    ScaleDownSizeWidth,
    ScaleDownSizeHeight,
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
        .expect("The iter is not emptyy");

        for i in from..to {
            paths.push(self.paths[i].path.clone());
        }
        paths
    }

    fn tag_of(&self, path: &str) -> Option<Tag> {
        self.paths
            .iter()
            .find(|info| info.path == path)
            .and_then(|info| info.metadata.tag.clone())
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
    GoToSorting,
    MoveImagesWithTag(Tag),
    FocusElement(widget::text_input::Id),
}

impl Model {
    fn new() -> (Self, Effect) {
        (
            Self {
                config: Config {
                    preload_back_num: 10,
                    preload_front_num: 30,
                    scale_down_size: (800, 600),
                },
                state: ModelState::LoadingListDir,
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
                    tag_names: HashMap::from_iter([
                        (TAG1, "Red".to_owned()),
                        (TAG2, "Green".to_owned()),
                        (TAG3, "Yellow".to_owned()),
                        (TAG4, "Blue".to_owned()),
                    ]),
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
        debug!("Message: {:?}", message);
        let effect = match message {
            Message::UserPressedGoToSettings => {
                let fields = HashMap::from_iter([
                    (
                        SettingsFieldName::PreloadBackNum,
                        (self.config.preload_back_num.to_string(), "".to_owned()),
                    ),
                    (
                        SettingsFieldName::PreloadFrontNum,
                        (self.config.preload_front_num.to_string(), "".to_owned()),
                    ),
                    (
                        SettingsFieldName::ScaleDownSizeWidth,
                        (self.config.scale_down_size.0.to_string(), "".to_owned()),
                    ),
                    (
                        SettingsFieldName::ScaleDownSizeHeight,
                        (self.config.scale_down_size.1.to_string(), "".to_owned()),
                    ),
                ]);
                self.state = ModelState::Settings(SettingsModel { fields });
                Effect::None
            }
            Message::UserPressedGoToSorting => {
                self.state = ModelState::LoadingListDir;
                Effect::LsDir
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
                ModelState::Sorting(model) => Model::update_sorting_model(
                    model,
                    SortingMessage::KeyboardEvent(event),
                    &self.config,
                ),
                _ => Effect::None,
            },
            Message::Sorting(sorting_message) => match &mut self.state {
                ModelState::Sorting(model) => {
                    Model::update_sorting_model(model, sorting_message, &self.config)
                }
                _ => Effect::None,
            },
            Message::Settings(settings_message) => match &mut self.state {
                ModelState::Settings(settings_model) => {
                    Model::update_settings_model(settings_model, settings_message, &mut self.config)
                }
                _ => panic!("Settings message ({settings_message:?}) in non-settings state"),
            },
        };

        debug!("Effect: {:?}", effect);
        effect
    }

    fn update_settings_model(
        model: &mut SettingsModel,
        message: SettingsMessage,
        _config: &mut Config,
    ) -> Effect {
        match message {
            SettingsMessage::UserUpdatedField(field, text) => {
                model.fields.insert(field, (text, "".to_owned()));
                Effect::None
            }
            SettingsMessage::UserPressedBackToSorting => Effect::GoToSorting,
            SettingsMessage::Save => {
                let (text, error) = model
                    .fields
                    .get_mut(&SettingsFieldName::PreloadBackNum)
                    .unwrap();
                match text.parse() {
                    Ok(num) => _config.preload_back_num = num,
                    Err(_) => *error = "Invalid number".to_owned(),
                }
                let (text, error) = model
                    .fields
                    .get_mut(&SettingsFieldName::PreloadFrontNum)
                    .unwrap();
                match text.parse() {
                    Ok(num) => _config.preload_front_num = num,
                    Err(_) => *error = "Invalid number".to_owned(),
                }
                let (text, error) = model
                    .fields
                    .get_mut(&SettingsFieldName::ScaleDownSizeWidth)
                    .unwrap();
                match text.parse() {
                    Ok(num) => _config.scale_down_size.0 = num,
                    Err(_) => *error = "Invalid number".to_owned(),
                }
                let (text, error) = model
                    .fields
                    .get_mut(&SettingsFieldName::ScaleDownSizeHeight)
                    .unwrap();
                match text.parse() {
                    Ok(num) => _config.scale_down_size.1 = num,
                    Err(_) => *error = "Invalid number".to_owned(),
                }
                Effect::None
            }
        }
    }

    #[allow(clippy::manual_inspect)]
    fn update_sorting_model(
        model: &mut SortingModel,
        message: SortingMessage,
        config: &Config,
    ) -> Effect {
        match message {
            SortingMessage::UserPressedPreviousImage => user_pressed_previous_image(model),
            SortingMessage::UserPressedNextImage => user_pressed_next_image(model),
            SortingMessage::ImagePreloadFailed(_path) => Effect::None,
            SortingMessage::ImagePreloaded(path, image) => {
                model
                    .pathlist
                    .paths
                    .iter_mut()
                    .find(|info| info.path == path)
                    .map(|info| {
                        info.data = PreloadImage::Loaded(image);
                        info
                    });

                schedule_next_preload_image_after_one_finished(&model.pathlist, config)
            }
            SortingMessage::KeyboardEvent(_) if is_typing_action(model) => Effect::None,
            SortingMessage::KeyboardEvent(event) => match event {
                iced::keyboard::Event::KeyPressed { key, modifiers, .. } => match key.as_ref() {
                    iced::keyboard::Key::Character("h")
                    | iced::keyboard::Key::Named(iced::keyboard::key::Named::ArrowLeft) => {
                        user_pressed_previous_image(model)
                    }
                    iced::keyboard::Key::Character("t" | "l")
                    | iced::keyboard::Key::Named(iced::keyboard::key::Named::ArrowRight) => {
                        user_pressed_next_image(model)
                    }
                    iced::keyboard::Key::Character(c)
                        if !modifiers.control() && TAGGING_CHARS.contains(c) =>
                    {
                        let tag = keybind_char_to_tag(c).unwrap();
                        // Any tagging character
                        Model::tag_and_move_on(model, tag)
                    }
                    iced::keyboard::Key::Named(iced::keyboard::key::Named::Delete) => {
                        Model::tag_and_move_on(model, TAG5)
                    }
                    iced::keyboard::Key::Named(iced::keyboard::key::Named::Backspace) => {
                        model.pathlist.paths[model.pathlist.index].metadata.tag = None;
                        Effect::None
                    }
                    _ => Effect::None,
                },
                _ => Effect::None,
            },
            SortingMessage::UserPressedTagButton(tag) => {
                Model::tag_and_move_on(model, tag);
                Effect::None
            }
            SortingMessage::UserPressedRenameTag(tag) => {
                let id = widget::text_input::Id::unique();
                model.editing_tag_name = Some((tag, "".to_owned(), id.clone()));
                model.expanded_dropdown = None;
                Effect::FocusElement(id)
            }
            SortingMessage::UserPressedSubmitRenameTag => {
                let (tag, new_tag_name, _) = model.editing_tag_name.take().unwrap();
                model.tag_names.insert(tag, new_tag_name);
                Effect::None
            }
            SortingMessage::UserPressedCancelRenameTag => {
                model.editing_tag_name = None;
                Effect::None
            }
            SortingMessage::UserEditTagName(text) => {
                model.editing_tag_name.as_mut().unwrap().1 = text;

                Effect::None
            }
            SortingMessage::UserPressedMoveTag(tag) => {
                model.expanded_dropdown = None;
                Effect::MoveImagesWithTag(tag)
            }
            SortingMessage::UserPressedTagMenu(maybe_tag) => {
                if model.expanded_dropdown.as_ref() == maybe_tag.as_ref() {
                    model.expanded_dropdown = None;
                } else {
                    model.expanded_dropdown = maybe_tag;
                }
                Effect::None
            }
        }
    }

    fn tag_and_move_on(model: &mut SortingModel, tag: Tag) -> Effect {
        model.pathlist.current_mut().metadata.tag = Some(tag);
        user_pressed_next_image(model)
    }

    fn view(&self) -> Element<Message> {
        match &self.state {
            ModelState::Sorting(model) => Model::view_sorting_model(model, &self.config),
            ModelState::LoadingListDir => widget::text("Loading...").into(),
            ModelState::EmptyDirectory => Model::view_empty_dir_model(),
            ModelState::Settings(settings_model) => Model::view_settings_model(settings_model),
        }
    }
    fn view_empty_dir_model() -> Element<'static, Message> {
        column![
            widget::text("No pictures in this directory, select another one"),
            button("Select Folder").on_press(Message::UserPressedSelectFolder),
        ]
        .into()
    }

    fn view_settings_model(model: &SettingsModel) -> Element<Message> {
        let (preload_back_text, preload_back_error) = model
            .fields
            .get(&SettingsFieldName::PreloadBackNum)
            .unwrap();
        let (preload_front_text, preload_front_error) = model
            .fields
            .get(&SettingsFieldName::PreloadFrontNum)
            .unwrap();
        let (scale_down_width_text, scale_down_width_error) = model
            .fields
            .get(&SettingsFieldName::ScaleDownSizeWidth)
            .unwrap();
        let (scale_down_height_text, scale_down_height_error) = model
            .fields
            .get(&SettingsFieldName::ScaleDownSizeHeight)
            .unwrap();

        column![
            widget::text("Settings"),
            row![
                widget::text("Preload back"),
                widget::text_input("Preload back", preload_back_text)
                    .id("preload_back_num")
                    .on_input(|text| Message::Settings(SettingsMessage::UserUpdatedField(
                        SettingsFieldName::PreloadBackNum,
                        text
                    ))),
                widget::text(preload_back_error)
            ],
            row![
                widget::text("Preload front"),
                widget::text_input("Preload front", preload_front_text)
                    .id("preload_front_num")
                    .on_input(|text| Message::Settings(SettingsMessage::UserUpdatedField(
                        SettingsFieldName::PreloadFrontNum,
                        text
                    ))),
                widget::text(preload_front_error),
            ],
            row![
                widget::text("Scale down size WxH"),
                widget::text_input("Width", scale_down_width_text)
                    .id("scale_down_size_width")
                    .on_input(|text| Message::Settings(SettingsMessage::UserUpdatedField(
                        SettingsFieldName::ScaleDownSizeWidth,
                        text
                    ))),
                widget::text(scale_down_width_error),
                widget::text_input("Height", scale_down_height_text)
                    .id("scale_down_size_height")
                    .on_input(|text| Message::Settings(SettingsMessage::UserUpdatedField(
                        SettingsFieldName::ScaleDownSizeHeight,
                        text
                    ))),
                widget::text(scale_down_height_error),
            ],
            button("Back to sorting")
                .on_press(Message::Settings(SettingsMessage::UserPressedBackToSorting,)),
            button("Save").on_press(Message::Settings(SettingsMessage::Save,)),
        ]
        .into()
    }

    fn view_sorting_model<'a>(model: &'a SortingModel, config: &'a Config) -> Element<'a, Message> {
        let sorting_view_style = SortingViewStyle::Thumbnails;

        let main_image_view = view_image_with_thumbs(sorting_view_style, model, config);

        let preload_status_string = preload_list_status_string_pathlist(&model.pathlist);

        let mut tag_count = HashMap::new();

        for metadata in model.pathlist.paths.iter().map(|info| &info.metadata) {
            if let Some(tag) = metadata.tag.clone() {
                let count = tag_count.entry(tag).or_insert(0);
                *count += 1;
            }
        }

        let status_text = widget::text(format!(
            "({index}/{total}) {path}",
            index = model.pathlist.index + 1,
            total = model.pathlist.paths.len(),
            path = model.pathlist.current().path,
        ));

        let tag_buttons = view_tag_button_row(
            model.expanded_dropdown.clone(),
            &model.tag_names,
            &tag_count,
        );

        let action_buttons = row![
            widget::button(widget::text!("{}", t!("<- Previous")))
                .on_press(Message::Sorting(SortingMessage::UserPressedPreviousImage))
                .padding(10),
            widget::button(widget::text!("{}", t!("Next ->")))
                .on_press(Message::Sorting(SortingMessage::UserPressedNextImage))
                .padding(10),
            widget::button(widget::text!("{}", t!("Settings")))
                .on_press(Message::UserPressedGoToSettings)
                .padding(10),
            widget::button(widget::text!("{}", t!("Select Folder")))
                .on_press(Message::UserPressedSelectFolder)
                .padding(10),
        ];

        let content = column![
            main_image_view,
            status_text,
            tag_buttons,
            action_buttons,
            widget::text(preload_status_string),
        ];

        let content = center(content);

        let popup = model
            .editing_tag_name
            .as_ref()
            .map(|(_, text, id)| view_rename_tag_modal(text.as_str(), id.clone()));

        stack![content].push_maybe(popup).into()
    }
}

enum SortingViewStyle {
    Thumbnails,
    #[allow(unused)]
    BeforeAfter,
}

#[derive(Clone)]
struct Dim {
    width: u32,
    height: u32,
}

fn view_image_with_thumbs<'a>(
    sorting_view_style: SortingViewStyle,
    model: &'a SortingModel,
    config: &'a Config,
) -> Element<'a, Message> {
    let img_dim = Dim {
        width: config.scale_down_size.0,
        height: config.scale_down_size.1,
    };
    let thumbs_dim = Dim {
        width: 100,
        height: 100,
    };
    match sorting_view_style {
        SortingViewStyle::BeforeAfter => {
            let prev_image = model
                .pathlist
                .prev()
                .map(|image| view_image(image, &model.tag_names, thumbs_dim.clone(), false))
                .unwrap_or(placeholder_text("No previous image", &thumbs_dim).into());

            let image = view_image(model.pathlist.current(), &model.tag_names, img_dim, false);

            let next_image = model
                .pathlist
                .next()
                .map(|image| view_image(image, &model.tag_names, thumbs_dim.clone(), false))
                .unwrap_or(placeholder_text("No next image", &thumbs_dim).into());

            row![prev_image, image, next_image].into()
        }
        SortingViewStyle::Thumbnails => {
            let image = view_image(model.pathlist.current(), &model.tag_names, img_dim, false);

            let num_thumbs = 3;
            let mut thumbs = Vec::new();
            for i in (model.pathlist.index as isize) - num_thumbs
                ..=(model.pathlist.index as isize) + num_thumbs
            {
                let img = if i >= 0 && i < model.pathlist.paths.len() as isize {
                    Some(&model.pathlist.paths[i as usize])
                } else {
                    None
                };

                let highlight = i == model.pathlist.index as isize;

                let thumb = img
                    .map(|image| view_image(image, &model.tag_names, thumbs_dim.clone(), highlight))
                    .unwrap_or(placeholder_text("No thumbnail", &thumbs_dim).into());
                thumbs.push(thumb);
            }

            column![widget::Row::from_vec(thumbs), image].into()
        }
    }
}

fn schedule_next_preload_image_after_one_finished(pathlist: &PathList, _config: &Config) -> Effect {
    let curr = pathlist.index;

    let forward = pathlist.paths.iter().skip(curr);
    let rev = pathlist
        .paths
        .iter()
        .rev()
        .skip(pathlist.paths.len() - curr);

    for e in forward.interleave(rev) {
        if matches!(e.data, PreloadImage::OutOfRange) {
            return Effect::PreloadImages(vec![e.path.to_owned()]);
        }
    }
    Effect::None
}

fn placeholder_text<'a>(msg: impl AsRef<str> + 'a, dim: &Dim) -> widget::Text<'a> {
    widget::text(msg.as_ref().to_owned())
        .width(dim.width as f32)
        .height(dim.height as f32)
}

fn keybind_char_to_tag(c: &str) -> Option<Tag> {
    match c {
        "a" => Some(TAG1),
        "o" => Some(TAG2),
        "e" => Some(TAG3),
        "u" => Some(TAG4),
        _ => None,
    }
}

struct TagColors {
    red: Color,
    green: Color,
    yellow: Color,
    blue: Color,
    other: Color,
}

const TAG_COLORS: TagColors = TagColors {
    red: Color::from_rgb(1.0, 0.0, 0.0),
    green: Color::from_rgb(0.0, 0.6, 0.0),
    yellow: Color::from_rgb(0.8, 0.8, 0.0),
    blue: Color::from_rgb(0.0, 0.0, 1.0),
    other: Color::from_rgb(0.5, 0.5, 0.5),
};

fn tag_badge_color(tag: &Tag) -> iced::Color {
    match *tag {
        TAG1 => TAG_COLORS.red,
        TAG2 => TAG_COLORS.green,
        TAG3 => TAG_COLORS.yellow,
        TAG4 => TAG_COLORS.blue,
        _ => TAG_COLORS.other,
    }
}

fn view_image<'a>(
    image: &'a ImageInfo,
    tag_names: &HashMap<Tag, String>,
    dim: Dim,
    highlight: bool,
) -> Element<'a, Message> {
    let name_and_color = image.metadata.tag.as_ref().map(|tag| {
        let name = tag_names.get(tag).unwrap();
        let color = tag_badge_color(tag);
        (name.to_owned(), color)
    });
    match &image.data {
        PreloadImage::Loaded(image) => view_loaded_image(image, name_and_color, dim, highlight),
        PreloadImage::Loading(path) => placeholder_text(format!("Loading {path}..."), &dim).into(),
        PreloadImage::OutOfRange => placeholder_text("Out of range", &dim).into(),
    }
}

fn is_typing_action(model: &SortingModel) -> bool {
    model.editing_tag_name.is_some()
}

fn view_rename_tag_modal(text: &str, id: widget::text_input::Id) -> Element<Message> {
    let input = widget::text_input("tag name", text)
        .on_input(|text| Message::Sorting(SortingMessage::UserEditTagName(text)))
        .on_submit(Message::Sorting(SortingMessage::UserPressedSubmitRenameTag))
        .id(id.clone());

    let submit =
        button("Submit").on_press(Message::Sorting(SortingMessage::UserPressedSubmitRenameTag));

    let cancel =
        button("Cancel").on_press(Message::Sorting(SortingMessage::UserPressedCancelRenameTag));

    column![input, row![submit, cancel,]]
        .spacing(20)
        .spacing(10)
        .padding(50)
        .into()
}

#[derive(Debug, Eq, Hash, PartialEq, Clone)]
enum Tag {
    Tag1,
    Tag2,
    Tag3,
    Tag4,
    Tag5,
}

const TAG1: Tag = Tag::Tag1;
const TAG2: Tag = Tag::Tag2;
const TAG3: Tag = Tag::Tag3;
const TAG4: Tag = Tag::Tag4;
const TAG5: Tag = Tag::Tag5;

fn view_tag_button_row<'a>(
    expanded: Option<Tag>,
    names: &'a HashMap<Tag, String>,
    nums: &HashMap<Tag, u32>,
) -> Element<'a, Message> {
    let red = names.get(&TAG1).map(|s| s.as_str()).unwrap_or("Red");
    let green = names.get(&TAG2).map(|s| s.as_str()).unwrap_or("Green");
    let yellow = names.get(&TAG3).map(|s| s.as_str()).unwrap_or("Yellow");
    let blue = names.get(&TAG4).map(|s| s.as_str()).unwrap_or("Blue");
    let red_num = *nums.get(&TAG1).unwrap_or(&0);
    let green_num = *nums.get(&TAG2).unwrap_or(&0);
    let yellow_num = *nums.get(&TAG3).unwrap_or(&0);
    let blue_num = *nums.get(&TAG4).unwrap_or(&0);
    row![
        view_tag_button(
            red,
            &TAG1,
            red_num,
            Color::from_rgb(1.0, 0.0, 0.0),
            Color::from_rgb(1.0, 0.4, 0.4),
            Color::from_rgb(5.0, 0.0, 0.0),
            expanded == Some(TAG1),
        ),
        view_tag_button(
            green,
            &TAG2,
            green_num,
            Color::from_rgb(0.0, 0.6, 0.0),
            Color::from_rgb(0.2, 6.0, 0.2),
            Color::from_rgb(0.0, 0.3, 0.0),
            expanded == Some(TAG2),
        ),
        view_tag_button(
            yellow,
            &TAG3,
            yellow_num,
            Color::from_rgb(0.8, 0.8, 0.0),
            Color::from_rgb(0.8, 0.8, 0.6),
            Color::from_rgb(0.3, 0.3, 0.0),
            expanded == Some(TAG3),
        ),
        view_tag_button(
            blue,
            &TAG4,
            blue_num,
            Color::from_rgb(0.0, 0.0, 1.0),
            Color::from_rgb(0.4, 0.4, 1.0),
            Color::from_rgb(0.0, 0.0, 0.5),
            expanded == Some(TAG4),
        ),
    ]
    .into()
}

fn view_tag_button<'a>(
    text: &'a str,
    tag: &Tag,
    num: u32,
    basic_bg: Color,
    hover_bg: Color,
    press_bg: Color,
    expanded: bool,
) -> Element<'a, Message> {
    let style = iced::widget::button::Style {
        background: Some(iced::Background::Color(basic_bg)),
        text_color: iced::Color::from_rgb(1.0, 1.0, 1.0),
        border: iced::Border::default(),
        shadow: iced::Shadow::default(),
    };
    let style_hovered = style.with_background(iced::Background::Color(hover_bg));

    let style_pressed = style.with_background(iced::Background::Color(press_bg));

    let tag_button = widget::Button::new(widget::text!("{text} ({num})\n[a]"))
        .style(move |_, status| match &status {
            widget::button::Status::Active => style,
            widget::button::Status::Hovered => style_hovered,
            widget::button::Status::Pressed => style_pressed,
            widget::button::Status::Disabled => style,
        })
        .on_press(Message::Sorting(SortingMessage::UserPressedTagButton(
            tag.clone(),
        )))
        .width(350)
        .height(55);

    let more_button = widget::button("...")
        .style(move |_, status| match &status {
            widget::button::Status::Active => style,
            widget::button::Status::Hovered => style_hovered,
            widget::button::Status::Pressed => style_pressed,
            widget::button::Status::Disabled => style,
        })
        .on_press(Message::Sorting(SortingMessage::UserPressedTagMenu(Some(
            tag.clone(),
        ))))
        .width(45)
        .height(55);

    let drop_down_menu = column![
        tag_dropdown_button(
            "Rename",
            SortingMessage::UserPressedRenameTag(tag.to_owned())
        ),
        tag_dropdown_button("Move", SortingMessage::UserPressedMoveTag(tag.to_owned())),
    ];

    let drop_down_button = DropDown::new(more_button, drop_down_menu, expanded)
        .alignment(drop_down::Alignment::Top)
        .on_dismiss(Message::Sorting(SortingMessage::UserPressedTagMenu(None)))
        .width(Length::Fill);

    row![tag_button, drop_down_button].into()
}

fn tag_dropdown_button(text: &str, message: SortingMessage) -> Element<Message> {
    button(text)
        .on_press(Message::Sorting(message))
        .width(250)
        .into()
}

fn preload_list_status_string_pathlist(list: &PathList) -> String {
    let mut s = String::new();
    for (index, info) in list.paths.iter().enumerate() {
        if index as isize >= list.index as isize - list.preload_back_num as isize
            && index <= list.index + list.preload_front_num
        {
            if index == list.index {
                s.push('[');
            }
            match info.data {
                PreloadImage::Loaded(_) => s.push('O'),
                PreloadImage::Loading(_) => s.push('x'),
                PreloadImage::OutOfRange => s.push(' '),
            }
            if index == list.index {
                s.push(']');
            }
        }
    }
    s
}

fn user_pressed_previous_image(model: &mut SortingModel) -> Effect {
    // We're already at the far left
    if model.pathlist.index == 0 {
        return Effect::None;
    }

    model.pathlist.index -= 1;

    if model.pathlist.index >= model.pathlist.preload_back_num {
        let new_preload_index =
            (model.pathlist.index as isize - model.pathlist.preload_back_num as isize) as usize;
        let info = &mut model.pathlist.paths[new_preload_index];
        if matches!(info.data, PreloadImage::OutOfRange) {
            info.data = PreloadImage::Loading(info.path.clone());
            Effect::PreloadImages(vec![info.path.clone()])
        } else {
            Effect::None
        }
    } else {
        Effect::None
    }
}

fn user_pressed_next_image(model: &mut SortingModel) -> Effect {
    // We're already at the far right
    if model.pathlist.index == model.pathlist.paths.len() - 1 {
        return Effect::None;
    }

    model.pathlist.index += 1;
    if model.pathlist.paths.len() > model.pathlist.index + model.pathlist.preload_front_num {
        let new_preload_index =
            (model.pathlist.index as isize + model.pathlist.preload_front_num as isize) as usize;
        let info = &mut model.pathlist.paths[new_preload_index];
        if matches!(info.data, PreloadImage::OutOfRange) {
            info.data = PreloadImage::Loading(info.path.clone());
            Effect::PreloadImages(vec![info.path.clone()])
        } else {
            Effect::None
        }
    } else {
        Effect::None
    }
}

fn effect_to_task(effect: Effect, model: &Model, config: Config) -> Task<Message> {
    match effect {
        Effect::None => Task::none(),
        Effect::LsDir => ls_dir_task(PICTURE_DIR.to_owned()),
        Effect::PreloadImages(paths) => preload_images_task(paths, config),
        Effect::GoToSorting => Task::done(Message::UserPressedGoToSorting),
        Effect::MoveImagesWithTag(tag) => {
            let (files_to_move, tag_name) = {
                let mut files_to_move = Vec::new();
                let tag_name = match &model.state {
                    ModelState::Sorting(sorting) => {
                        for info in &sorting.pathlist.paths {
                            if info.metadata.tag == Some(tag.clone()) {
                                files_to_move.push(info.path.clone());
                            }
                        }
                        sorting.tag_names.get(&tag).unwrap().clone()
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
                mv_files_task(files_to_move, tag_name)
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

fn view_loaded_image(
    image: &ImageData,
    name_and_color: Option<(String, iced::Color)>,
    dim: Dim,
    highlight: bool,
) -> Element<Message> {
    let mut img = iced::widget::image::viewer(widget::image::Handle::from_rgba(
        image.width,
        image.height,
        image.data.clone(),
    ));
    img = img.width(dim.width as f32).height(dim.height as f32);

    let image_with_border = if highlight {
        widget::container(img)
            .style(|_: &iced::Theme| {
                widget::container::Style::default().border(iced::Border {
                    radius: iced::border::radius(5),
                    color: Color::from_rgb(0.0, 0.2, 0.8),
                    width: 3.0,
                })
            })
            .padding(3)
    } else {
        widget::container(img)
    };

    let badge: Option<Element<Message>> = name_and_color.map(|(name, mut color)| {
        color.a = 0.75;
        widget::container(widget::text(name))
            .padding(10)
            .style(move |_: &iced::Theme| widget::container::Style {
                background: Some(iced::Background::Color(color)),
                border: iced::border::rounded(10.0),
                text_color: Some(Color::WHITE),
                ..widget::container::Style::default()
            })
            .into()
    });

    stack![image_with_border].push_maybe(badge).into()
}

fn button(text: &str) -> widget::Button<'_, Message> {
    widget::button(text).padding(10)
}

#[cfg(test)]
mod tests {}
