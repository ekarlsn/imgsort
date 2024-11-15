use std::collections::{HashMap, HashSet};

use iced::event::{self, Event};
use iced::widget::{self, center, column, row, text};
use iced::{Element, Subscription, Task};
use image::ImageReader;
use log::debug;

const TAGGING_CHARS: &str = "aoeupy";
const PICTURE_DIR: &str = ".";

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
    Sorting(SortingModel),
    Settings(SettingsModel),
}

#[derive(Debug)]
struct SortingModel {
    pathlist: PathList,
    preload_list: PreloadList,

    // Tags
    selected_tag: Option<String>,
    taglist_combobox_state: widget::combo_box::State<String>,

    // Action
    action_text: String,
    action_error_text: String,
    is_typing_action: bool,
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
struct PathList {
    paths: Vec<(String, Metadata)>,
    index: usize,
}

#[derive(Debug)]
struct Metadata {
    tag: Option<String>,
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

#[derive(Debug)]
struct PreloadList {
    images: Vec<PreloadImage>,
    preload_back_num: usize,
    preload_front_num: usize,
    index: usize,
}

#[derive(Debug, Clone)]
enum Message {
    UserPressedGoToSettings,
    UserPressedGoToSorting,
    ListDirCompleted(Vec<String>),
    KeyboardEventOccurred(iced::keyboard::Event),
    SettingsMessage(SettingsMessage),
    SortingMessage(SortingMessage),
}

#[derive(Debug, Clone)]
enum SortingMessage {
    UserPressedNextImage,
    UserPressedPreviousImage,
    UserSelectedTag(String),
    UserPressedMove,
    UserUpdatedActionText(String),
    UserSubmittedMove,
    UserAbortedAction,
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
    fn new(paths: Vec<String>) -> Self {
        let paths = paths
            .iter()
            .map(|path| (path.clone(), Metadata { tag: None }))
            .collect();
        Self { paths, index: 0 }
    }

    fn get_offset_index(&self, path: &str) -> Option<isize> {
        let target_index = self.paths.iter().position(|(p, _)| p == path);
        if let Some(target_index) = target_index {
            Some(target_index as isize - self.index as isize)
        } else {
            None
        }
    }

    fn tag_of(&self, path: &str) -> Option<String> {
        self.paths
            .iter()
            .find(|(p, _)| p == path)
            .map(|(_, meta)| meta.tag.clone())
            .flatten()
    }
}

impl PreloadList {
    fn new(
        preload_back_num: usize,
        preload_front_num: usize,
        paths: Vec<String>,
    ) -> (Self, Vec<String>) {
        let mut images = Vec::new();
        for _ in 0..preload_back_num {
            images.push(PreloadImage::OutOfRange);
        }
        for i in 0..=preload_front_num {
            images.push(PreloadImage::Loading(
                paths.get(i).unwrap_or(&"Incorrect path".to_owned()).clone(),
            ));
        }
        (
            Self {
                images,
                preload_back_num,
                preload_front_num,
                index: preload_back_num,
            },
            paths.iter().take(preload_front_num + 1).cloned().collect(),
        )
    }

    fn current_image(&self) -> &PreloadImage {
        &self.images[self.index]
    }

    fn image_loaded(&mut self, offset_index: isize, image: ImageData) {
        if offset_index >= -(self.preload_back_num as isize)
            && offset_index <= (self.preload_front_num as isize)
        {
            let index = (self.index as isize + offset_index).rem_euclid(self.images.len() as isize);
            let index: usize = index.try_into().unwrap();
            self.images[index] = PreloadImage::Loaded(image);
        }
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
    MoveImages(String),
    FocusMoveField,
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
        let (preload_list, preload_tasks) = PreloadList::new(
            self.config.preload_back_num,
            self.config.preload_front_num,
            paths.clone(),
        );
        let preload_images_effect = initial_preloads(preload_tasks);

        match &mut self.state {
            ModelState::Sorting(sorting) => {
                debug!("In sorting model, received new lsdir, updating");

                // Preload list
                sorting.preload_list = preload_list;

                // Pathlist
                let index: usize = {
                    if let Some(previous_image) = sorting
                        .pathlist
                        .paths
                        .get(sorting.pathlist.index)
                        .map(|p| &p.0)
                    {
                        paths.iter().position(|p| p == previous_image).unwrap_or(0)
                    } else {
                        0
                    }
                };

                let paths = paths
                    .iter()
                    .map(|path| {
                        (
                            path.clone(),
                            Metadata {
                                tag: sorting.pathlist.tag_of(path),
                            },
                        )
                    })
                    .collect();

                sorting.pathlist = PathList { index, paths };

                // Taglist combobox
                let all_tags = find_all_tags(sorting.pathlist.paths.as_slice());
                sorting.taglist_combobox_state = widget::combo_box::State::new(all_tags);
                sorting.selected_tag = None;
            }

            _ => {
                debug!("Going to new sorting model");

                self.state = ModelState::Sorting(SortingModel {
                    pathlist: PathList::new(paths.clone()),
                    preload_list,
                    selected_tag: None,
                    taglist_combobox_state: widget::combo_box::State::default(),
                    action_text: "".to_owned(),
                    action_error_text: "".to_owned(),
                    is_typing_action: false,
                });
            }
        }

        preload_images_effect
    }

    fn title(&self) -> String {
        format!("ImageViewer")
    }

    fn update_with_task(&mut self, message: Message) -> Task<Message> {
        effect_to_task(self.update(message), self, self.config.clone())
    }

    fn update(&mut self, message: Message) -> Effect {
        debug!("Message: {:?}", message);
        let effect = match message {
            Message::UserPressedGoToSettings => {
                let fields = HashMap::from_iter(
                    [
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
                    ]
                    .into_iter(),
                );
                self.state = ModelState::Settings(SettingsModel { fields });
                Effect::None
            }
            Message::UserPressedGoToSorting => {
                self.state = ModelState::LoadingListDir;
                Effect::LsDir
            }
            Message::ListDirCompleted(paths) => self.go_to_sorting_model(paths),
            Message::KeyboardEventOccurred(event) => match &mut self.state {
                ModelState::Sorting(model) => {
                    Model::update_sorting_model(model, SortingMessage::KeyboardEvent(event))
                }
                _ => Effect::None,
            },
            Message::SortingMessage(sorting_message) => match &mut self.state {
                ModelState::Sorting(model) => Model::update_sorting_model(model, sorting_message),
                _ => Effect::None,
            },
            Message::SettingsMessage(settings_message) => match &mut self.state {
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

    fn update_sorting_model(model: &mut SortingModel, message: SortingMessage) -> Effect {
        match message {
            SortingMessage::UserPressedPreviousImage => user_pressed_previous_image(model),
            SortingMessage::UserPressedNextImage => user_pressed_next_image(model),
            SortingMessage::ImagePreloadFailed(_path) => Effect::None,
            SortingMessage::ImagePreloaded(path, image) => {
                if let Some(offset_index) = model.pathlist.get_offset_index(&path) {
                    debug!("Offset index: {offset_index:?}");
                    model.preload_list.image_loaded(offset_index, image);
                }

                Effect::None
            }
            SortingMessage::KeyboardEvent(_) if model.is_typing_action => Effect::None,
            SortingMessage::KeyboardEvent(event) => match event {
                iced::keyboard::Event::KeyPressed { key, modifiers, .. } => match key.as_ref() {
                    iced::keyboard::Key::Character("h") => user_pressed_previous_image(model),
                    iced::keyboard::Key::Character("t" | "l") => user_pressed_next_image(model),
                    iced::keyboard::Key::Character(c)
                        if !modifiers.control() && TAGGING_CHARS.contains(c) =>
                    {
                        // Any tagging character
                        model.pathlist.paths[model.pathlist.index].1.tag = Some(c.to_owned());
                        let all_tags = find_all_tags(&model.pathlist.paths.as_slice());
                        model.taglist_combobox_state = widget::combo_box::State::new(all_tags);
                        user_pressed_next_image(model)
                    }
                    iced::keyboard::Key::Named(iced::keyboard::key::Named::Delete) => {
                        model.pathlist.paths[model.pathlist.index].1.tag = Some("D".to_owned());
                        let all_tags = find_all_tags(&model.pathlist.paths.as_slice());
                        model.taglist_combobox_state = widget::combo_box::State::new(all_tags);
                        user_pressed_next_image(model)
                    }
                    iced::keyboard::Key::Named(iced::keyboard::key::Named::Backspace) => {
                        model.pathlist.paths[model.pathlist.index].1.tag = None;
                        let all_tags = find_all_tags(&model.pathlist.paths.as_slice());
                        model.taglist_combobox_state = widget::combo_box::State::new(all_tags);
                        Effect::None
                    }
                    _ => Effect::None,
                },
                _ => Effect::None,
            },
            SortingMessage::UserSelectedTag(tag) => {
                model.selected_tag = Some(tag);
                Effect::None
            }
            SortingMessage::UserUpdatedActionText(text) => {
                model.action_text = text;

                Effect::None
            }
            SortingMessage::UserPressedMove => {
                model.is_typing_action = true;
                Effect::FocusMoveField
            }
            SortingMessage::UserSubmittedMove => {
                if model.action_text.is_empty() {
                    model.action_error_text =
                        "Type the destination folder in the text box".to_owned();
                    Effect::None
                } else {
                    model.action_error_text = "".to_owned();
                    model.is_typing_action = false;
                    Effect::MoveImages(model.action_text.clone())
                }
            }
            SortingMessage::UserAbortedAction => {
                model.is_typing_action = false;
                model.action_error_text = "".to_owned();
                model.action_text = "".to_owned();

                Effect::None
            }
        }
    }

    fn view(&self) -> Element<Message> {
        match &self.state {
            ModelState::Sorting(model) => Model::view_sorting_model(model),
            ModelState::LoadingListDir => text("Loading...").into(),
            ModelState::Settings(settings_model) => Model::view_settings_model(settings_model),
        }
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
            text("Settings"),
            row![
                text("Preload back"),
                widget::text_input("Preload back", preload_back_text)
                    .id("preload_back_num")
                    .on_input(
                        |text| Message::SettingsMessage(SettingsMessage::UserUpdatedField(
                            SettingsFieldName::PreloadBackNum,
                            text
                        ))
                    ),
                text(preload_back_error)
            ],
            row![
                text("Preload front"),
                widget::text_input("Preload front", preload_front_text)
                    .id("preload_front_num")
                    .on_input(
                        |text| Message::SettingsMessage(SettingsMessage::UserUpdatedField(
                            SettingsFieldName::PreloadFrontNum,
                            text
                        ))
                    ),
                text(preload_front_error),
            ],
            row![
                text("Scale down size WxH"),
                widget::text_input("Width", scale_down_width_text)
                    .id("scale_down_size_width")
                    .on_input(
                        |text| Message::SettingsMessage(SettingsMessage::UserUpdatedField(
                            SettingsFieldName::ScaleDownSizeWidth,
                            text
                        ))
                    ),
                widget::text(scale_down_width_error),
                widget::text_input("Height", scale_down_height_text)
                    .id("scale_down_size_height")
                    .on_input(
                        |text| Message::SettingsMessage(SettingsMessage::UserUpdatedField(
                            SettingsFieldName::ScaleDownSizeHeight,
                            text
                        ))
                    ),
                widget::text(scale_down_height_error),
            ],
            button("Back to sorting").on_press(Message::SettingsMessage(
                SettingsMessage::UserPressedBackToSorting,
            )),
            button("Save").on_press(Message::SettingsMessage(SettingsMessage::Save,)),
        ]
        .into()
    }

    fn view_sorting_model(model: &SortingModel) -> Element<Message> {
        let image: Element<_> = match model.preload_list.current_image() {
            PreloadImage::Loaded(image) => column![
                view_image(&image),
                text(format!(
                    "Image {index}/{total}",
                    index = model.pathlist.index + 1,
                    total = model.pathlist.paths.len()
                )),
                text(format!(
                    "Path: {path}",
                    path = model.pathlist.paths[model.pathlist.index].0
                ))
            ]
            .into(),
            PreloadImage::Loading(path) => text(format!("Loading {path}...")).into(),
            PreloadImage::OutOfRange => text("Out of range").into(),
        };

        let preload_status_string = preload_list_status_string(&model.preload_list);

        let tag = model.pathlist.paths[model.pathlist.index].1.tag.clone();

        let actions: Element<_> = if model.selected_tag.is_some() {
            if model.is_typing_action {
                let action_text_input = column![
                    widget::text_input("Action text", &model.action_text).on_input(|text| {
                        Message::SortingMessage(SortingMessage::UserUpdatedActionText(text))
                    }),
                    row![
                        button("Submit")
                            .on_press(Message::SortingMessage(SortingMessage::UserSubmittedMove)),
                        button("Abort")
                            .on_press(Message::SortingMessage(SortingMessage::UserAbortedAction)),
                    ],
                ];
                if model.action_error_text.is_empty() {
                    action_text_input.into()
                } else {
                    column![
                        text(&model.action_error_text).color(iced::Color::from_rgb(1.0, 0.0, 0.0)),
                        action_text_input,
                    ]
                    .into()
                }
            } else {
                button("Move")
                    .on_press(Message::SortingMessage(SortingMessage::UserPressedMove))
                    .into()
            }
        } else {
            text("Select a tag for actions ...").into()
        };

        let content = column![
            image,
            match tag {
                Some(tag) => text(format!("Tag: [{tag}]")),
                None => text("No tag"),
            },
            row![
                button("<- Previous").on_press(Message::SortingMessage(
                    SortingMessage::UserPressedPreviousImage
                )),
                button("Next ->").on_press(Message::SortingMessage(
                    SortingMessage::UserPressedNextImage
                )),
                button("Settings").on_press(Message::UserPressedGoToSettings),
            ],
            text(preload_status_string),
            widget::combo_box(
                &model.taglist_combobox_state,
                "Select a tag",
                model.selected_tag.as_ref(),
                |tag| Message::SortingMessage(SortingMessage::UserSelectedTag(tag))
            ),
            actions,
        ];

        center(content).into()
    }
}

fn find_all_tags(paths: &[(String, Metadata)]) -> Vec<String> {
    let mut tags = paths
        .iter()
        .filter_map(|(_, meta)| meta.tag.as_ref())
        .collect::<HashSet<&String>>()
        .into_iter()
        .cloned()
        .collect::<Vec<String>>();
    tags.sort();
    tags
}

fn preload_list_status_string(list: &PreloadList) -> String {
    let preload_state_to_string = |preload_state: &PreloadImage| match preload_state {
        PreloadImage::Loaded(_) => "O",
        PreloadImage::Loading(_) => "x",
        PreloadImage::OutOfRange => " ",
    };

    let make_preleoad_status_string = |slice: &[PreloadImage]| {
        slice
            .iter()
            .map(preload_state_to_string)
            .collect::<String>()
    };

    let me = preload_state_to_string(&list.images[list.index]).to_owned();
    let me = format!("[{me}]");
    if list.index < list.preload_back_num {
        // The left side goes over the edge
        let left1 =
            make_preleoad_status_string(&list.images[(list.index + list.preload_front_num) + 1..]);
        let left2 = make_preleoad_status_string(&list.images[..list.index]);

        let right = make_preleoad_status_string(
            &list.images[(list.index + 1)..list.index + list.preload_front_num + 1],
        );

        vec![left1, left2, me, right].join("")
    } else {
        // The right side goes over the edge
        let left = make_preleoad_status_string(
            &list.images[(list.index - list.preload_back_num)..list.index],
        );

        let right1 = make_preleoad_status_string(&list.images[(list.index + 1)..]);
        let right2 =
            make_preleoad_status_string(&list.images[..list.index - list.preload_back_num]);

        vec![left, me, right1, right2].join("")
    }
}

fn user_pressed_previous_image(model: &mut SortingModel) -> Effect {
    // We're already at the far left
    if model.pathlist.index == 0 {
        return Effect::None;
    }

    model.pathlist.index = model.pathlist.index - 1;
    model.preload_list.index = (model.preload_list.index as isize - 1)
        .rem_euclid(model.preload_list.images.len() as isize)
        .try_into()
        .unwrap();

    let index_of_next_image_to_preload =
        model.pathlist.index as isize - model.preload_list.preload_back_num as isize;

    let preload_index = (model.preload_list.index as isize
        - model.preload_list.preload_back_num as isize)
        .rem_euclid(model.preload_list.images.len() as isize);

    let (preload_image_state, effect) = if index_of_next_image_to_preload < 0 {
        // The new index to preload is out of bounds
        (PreloadImage::OutOfRange, Effect::None)
    } else {
        let new_preload_image = model.pathlist.paths[index_of_next_image_to_preload as usize]
            .0
            .clone();
        (
            PreloadImage::Loading(new_preload_image.clone()),
            Effect::PreloadImages(vec![new_preload_image]),
        )
    };

    model.preload_list.images[preload_index as usize] = preload_image_state;
    effect
}

fn user_pressed_next_image(model: &mut SortingModel) -> Effect {
    // We're already at the far right
    if model.pathlist.index == model.pathlist.paths.len() - 1 {
        return Effect::None;
    }

    model.pathlist.index = model.pathlist.index + 1;
    model.preload_list.index =
        (model.preload_list.index + 1).rem_euclid(model.preload_list.images.len());

    let index_of_next_image_to_preload =
        model.pathlist.index + model.preload_list.preload_front_num;

    let preload_index = (model.preload_list.index + model.preload_list.preload_front_num)
        .rem_euclid(model.preload_list.images.len());

    let (preload_image_state, effect) =
        if index_of_next_image_to_preload >= model.pathlist.paths.len() {
            // The new index to preload is out of bounds
            (PreloadImage::OutOfRange, Effect::None)
        } else {
            let new_preload_image = model.pathlist.paths[index_of_next_image_to_preload]
                .0
                .clone();
            (
                PreloadImage::Loading(new_preload_image.clone()),
                Effect::PreloadImages(vec![new_preload_image]),
            )
        };

    model.preload_list.images[preload_index as usize] = preload_image_state;
    effect
}

fn effect_to_task(effect: Effect, model: &Model, config: Config) -> Task<Message> {
    match effect {
        Effect::None => Task::none(),
        Effect::LsDir => ls_dir_task(PICTURE_DIR.to_owned()),
        Effect::PreloadImages(paths) => preload_images_task(paths, config),
        Effect::GoToSorting => Task::done(Message::UserPressedGoToSorting),
        Effect::MoveImages(destination) => {
            println!("I won't actually move any images, but I'll print a command for you");
            let mut files_to_move = Vec::new();
            match &model.state {
                ModelState::Sorting(sorting) => {
                    if let Some(tag) = &sorting.selected_tag {
                        for (path, meta) in &sorting.pathlist.paths {
                            if meta.tag == Some(tag.clone()) {
                                files_to_move.push(path.clone());
                            }
                        }
                    }
                }
                _ => panic!("MoveImages effect should only be called in the sorting state"),
            }
            if files_to_move.is_empty() {
                println!("No files to move");
                Task::none()
            } else {
                println!("mv {} {}", files_to_move.join(" "), destination);
                mv_files_task(files_to_move, destination)
                    .then(|()| ls_dir_task(PICTURE_DIR.to_owned()))
            }
        }
        Effect::FocusMoveField => {
            // TODO
            Task::none()
        }
    }
}

fn initial_preloads(paths: Vec<String>) -> Effect {
    Effect::PreloadImages(paths)
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
            Ok((path4, image)) => {
                Message::SortingMessage(SortingMessage::ImagePreloaded(path4, image))
            }
            Err(_) => Message::SortingMessage(SortingMessage::ImagePreloadFailed(
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

fn view_image(image: &ImageData) -> Element<Message> {
    iced::widget::image::viewer(widget::image::Handle::from_rgba(
        image.width,
        image.height,
        image.data.clone(),
    ))
    .into()
}

fn button(text: &str) -> widget::Button<'_, Message> {
    widget::button(text).padding(10)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    enum PreloadExpect {
        Loading(String),
        Loaded,
        OutOfRange,
    }

    fn expect_preloads(model: &Model, expected_index: usize, expected: Vec<PreloadExpect>) {
        let actual = {
            match &model.state {
                ModelState::Sorting(sorting_model) => {
                    assert_eq!(sorting_model.preload_list.index, expected_index);
                    &sorting_model.preload_list.images
                }
                _ => panic!("Unexpected state"),
            }
        };
        assert_eq!(actual.len(), expected.len());
        for i in 0..actual.len() {
            match &actual[i] {
                PreloadImage::Loading(path) => match &expected[i] {
                    PreloadExpect::Loading(expected_path) => {
                        assert_eq!(path, expected_path);
                    }
                    expectation => panic!(
                        "Expected element {i} to be {expectation:?}, but it was {:?}",
                        actual[i]
                    ),
                },
                PreloadImage::Loaded(_image) => match &expected[i] {
                    PreloadExpect::Loaded => {}
                    expectation => panic!(
                        "Expected element {i} to be {expectation:?}, but it was {:?}",
                        actual[i]
                    ),
                },
                PreloadImage::OutOfRange => match &expected[i] {
                    PreloadExpect::OutOfRange => {}
                    expectation => panic!(
                        "Expected element {i} to be {expectation:?}, but it was {:?}",
                        actual[i]
                    ),
                },
            }
        }
    }

    #[test]
    fn test_preload_string() {
        let img = ImageData {
            width: 1,
            height: 1,
            data: vec![],
        };
        for (i, expected) in [
            "__[_]OOO", "__[O]OO_", "_O[O]O__", "OO[O]___", "OO[_]__O", "O_[_]_OO",
        ]
        .iter()
        .enumerate()
        {
            let list = PreloadList {
                index: i,
                images: vec![
                    PreloadImage::Loading("pictures/real/1.jpg".to_owned()),
                    PreloadImage::Loaded(img.clone()),
                    PreloadImage::Loaded(img.clone()),
                    PreloadImage::Loaded(img.clone()),
                    PreloadImage::Loading("pictures/real/5.jpg".to_owned()),
                    PreloadImage::Loading("pictures/real/6.jpg".to_owned()),
                ],
                preload_back_num: 2,
                preload_front_num: 3,
            };
            assert_eq!(
                preload_list_status_string(&list),
                *expected,
                "Tested with index {i}"
            )
        }
    }

    fn preloaded_message(name: &str) -> Message {
        Message::SortingMessage(SortingMessage::ImagePreloaded(
            name.to_owned(),
            ImageData {
                width: 1,
                height: 1,
                data: vec![],
            },
        ))
    }

    #[test]
    fn test_flow() {
        simplelog::TermLogger::init(
            simplelog::LevelFilter::Debug,
            simplelog::ConfigBuilder::new()
                .add_filter_allow_str("imgsort")
                .build(),
            simplelog::TerminalMode::Mixed,
            simplelog::ColorChoice::Auto,
        )
        .unwrap();

        let next_image = Message::SortingMessage(SortingMessage::UserPressedNextImage);
        let prev_image = Message::SortingMessage(SortingMessage::UserPressedPreviousImage);

        let (mut model, effect) = Model::new();
        assert_eq!(effect, Effect::LsDir);

        let effect = model.update(Message::ListDirCompleted(vec![
            "pictures/real/1.jpg".to_owned(),
            "pictures/real/2.jpg".to_owned(),
            "pictures/real/3.jpg".to_owned(),
            "pictures/real/4.jpg".to_owned(),
            "pictures/real/5.jpg".to_owned(),
            "pictures/real/6.jpg".to_owned(),
        ]));

        assert_eq!(
            effect,
            Effect::PreloadImages(vec![
                "pictures/real/1.jpg".to_owned(),
                "pictures/real/2.jpg".to_owned(),
                "pictures/real/3.jpg".to_owned(),
            ])
        );

        expect_preloads(
            &model,
            1,
            vec![
                PreloadExpect::OutOfRange,
                PreloadExpect::Loading("pictures/real/1.jpg".to_owned()),
                PreloadExpect::Loading("pictures/real/2.jpg".to_owned()),
                PreloadExpect::Loading("pictures/real/3.jpg".to_owned()),
            ],
        );

        let effect = model.update(preloaded_message("pictures/real/2.jpg"));

        assert_eq!(effect, Effect::None);

        expect_preloads(
            &model,
            1,
            vec![
                PreloadExpect::OutOfRange,
                PreloadExpect::Loading("pictures/real/1.jpg".to_owned()),
                PreloadExpect::Loaded,
                PreloadExpect::Loading("pictures/real/3.jpg".to_owned()),
            ],
        );

        assert_eq!(
            model.update(next_image.clone()),
            Effect::PreloadImages(vec!["pictures/real/4.jpg".to_owned(),])
        );

        expect_preloads(
            &model,
            2,
            vec![
                PreloadExpect::Loading("pictures/real/4.jpg".to_owned()),
                PreloadExpect::Loading("pictures/real/1.jpg".to_owned()),
                PreloadExpect::Loaded,
                PreloadExpect::Loading("pictures/real/3.jpg".to_owned()),
            ],
        );

        assert_eq!(model.update(prev_image.clone()), Effect::None,);

        expect_preloads(
            &model,
            1,
            vec![
                PreloadExpect::OutOfRange,
                PreloadExpect::Loading("pictures/real/1.jpg".to_owned()),
                PreloadExpect::Loaded,
                PreloadExpect::Loading("pictures/real/3.jpg".to_owned()),
            ],
        );

        // Already at first image, should change nothing
        assert_eq!(model.update(prev_image.clone()), Effect::None,);
        expect_preloads(
            &model,
            1,
            vec![
                PreloadExpect::OutOfRange,
                PreloadExpect::Loading("pictures/real/1.jpg".to_owned()),
                PreloadExpect::Loaded,
                PreloadExpect::Loading("pictures/real/3.jpg".to_owned()),
            ],
        );

        assert_eq!(
            model.update(next_image.clone()),
            Effect::PreloadImages(vec!["pictures/real/4.jpg".to_owned()]),
        );
        expect_preloads(
            &model,
            2,
            vec![
                PreloadExpect::Loading("pictures/real/4.jpg".to_owned()),
                PreloadExpect::Loading("pictures/real/1.jpg".to_owned()),
                PreloadExpect::Loaded,
                PreloadExpect::Loading("pictures/real/3.jpg".to_owned()),
            ],
        );

        assert_eq!(
            model.update(next_image.clone()),
            Effect::PreloadImages(vec!["pictures/real/5.jpg".to_owned()]),
        );
        expect_preloads(
            &model,
            3,
            vec![
                PreloadExpect::Loading("pictures/real/4.jpg".to_owned()),
                PreloadExpect::Loading("pictures/real/5.jpg".to_owned()),
                PreloadExpect::Loaded,
                PreloadExpect::Loading("pictures/real/3.jpg".to_owned()),
            ],
        );

        assert_eq!(
            model.update(next_image.clone()),
            Effect::PreloadImages(vec!["pictures/real/6.jpg".to_owned()]),
        );
        expect_preloads(
            &model,
            0,
            vec![
                PreloadExpect::Loading("pictures/real/4.jpg".to_owned()),
                PreloadExpect::Loading("pictures/real/5.jpg".to_owned()),
                PreloadExpect::Loading("pictures/real/6.jpg".to_owned()),
                PreloadExpect::Loading("pictures/real/3.jpg".to_owned()),
            ],
        );

        // See the right end
        assert_eq!(model.update(next_image.clone()), Effect::None,);
        expect_preloads(
            &model,
            1,
            vec![
                PreloadExpect::Loading("pictures/real/4.jpg".to_owned()),
                PreloadExpect::Loading("pictures/real/5.jpg".to_owned()),
                PreloadExpect::Loading("pictures/real/6.jpg".to_owned()),
                PreloadExpect::OutOfRange,
            ],
        );

        // At the last image
        assert_eq!(model.update(next_image.clone()), Effect::None,);
        expect_preloads(
            &model,
            2,
            vec![
                PreloadExpect::OutOfRange,
                PreloadExpect::Loading("pictures/real/5.jpg".to_owned()),
                PreloadExpect::Loading("pictures/real/6.jpg".to_owned()),
                PreloadExpect::OutOfRange,
            ],
        );

        // Trying to go past the last image
        assert_eq!(model.update(next_image.clone()), Effect::None,);
        expect_preloads(
            &model,
            2,
            vec![
                PreloadExpect::OutOfRange,
                PreloadExpect::Loading("pictures/real/5.jpg".to_owned()),
                PreloadExpect::Loading("pictures/real/6.jpg".to_owned()),
                PreloadExpect::OutOfRange,
            ],
        );

        // Go back one
        assert_eq!(
            model.update(prev_image.clone()),
            Effect::PreloadImages(vec!["pictures/real/4.jpg".to_owned()]),
        );
        expect_preloads(
            &model,
            1,
            vec![
                PreloadExpect::Loading("pictures/real/4.jpg".to_owned()),
                PreloadExpect::Loading("pictures/real/5.jpg".to_owned()),
                PreloadExpect::Loading("pictures/real/6.jpg".to_owned()),
                PreloadExpect::OutOfRange,
            ],
        );
    }
}
