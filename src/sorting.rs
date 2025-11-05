use crate::ui::{self, ButtonStyle};
use iced::widget::{self, button, canvas, center, column, row, stack};
use iced::{Color, Element, Length};
use iced_aw::{drop_down, DropDown};
use log::debug;
use rust_i18n::t;
use std::cmp::min;
use std::collections::HashMap;

use crate::image_widget::PixelCanvas;
use crate::{
    Effect, ImageData, ImageInfo, LoadedImageAndThumb, Message, PathList, PreloadImage,
    SortingViewStyle,
};

// Constants
pub const TAGGING_CHARS: &str = "aoeupy";

#[derive(Debug, Clone)]
pub enum SortingMessage {
    UserPressedNextImage,
    UserPressedPreviousImage,
    UserPressedMoveTag(Tag),
    UserPressedTagButton(Tag),
    UserPressedRenameTag(Tag),
    UserPressedSubmitRenameTag,
    UserPressedCancelRenameTag,
    UserEditTagName(String),
    UserPressedTagMenu(Option<Tag>),
    ImagePreloaded(String, ImageData, ImageData),
    KeyboardEvent(iced::keyboard::Event),
    CanvasResized(Dim),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Copy)]
pub enum Tag {
    Tag1,
    Tag2,
    Tag3,
    Tag4,
    Tag5,
    Tag6,
    Tag7,
    Tag8,
}

#[derive(Debug, Clone)]
pub struct TagNames {
    pub tag1: String,
    pub tag2: String,
    pub tag3: String,
    pub tag4: String,
    pub tag5: String,
    pub tag6: String,
    pub tag7: String,
    pub tag8: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Dim {
    pub width: u32,
    pub height: u32,
}

struct TagColors {
    red: Color,
    green: Color,
    yellow: Color,
    blue: Color,
    purple: Color,
    orange: Color,
    gray: Color,
    cyan: Color,
}

const TAG_COLORS: TagColors = TagColors {
    red: Color::from_rgb(1.0, 0.0, 0.0),
    green: Color::from_rgb(0.0, 0.6, 0.0),
    yellow: Color::from_rgb(0.8, 0.8, 0.0),
    blue: Color::from_rgb(0.0, 0.0, 1.0),
    purple: Color::from_rgb(0.5, 0.0, 0.5),
    orange: Color::from_rgb(1.0, 0.5, 0.0),
    gray: Color::from_rgb(0.5, 0.5, 0.5),
    cyan: Color::from_rgb(0.0, 1.0, 1.0),
};

impl TagNames {
    pub fn new() -> Self {
        Self {
            tag1: String::from("Red"),
            tag2: String::from("Green"),
            tag3: String::from("Yellow"),
            tag4: String::from("Blue"),
            tag5: String::from("Purple"),
            tag6: String::from("Orange"),
            tag7: String::from("Gray"),
            tag8: String::from("Cyan"),
        }
    }

    pub fn update(&mut self, tag: Tag, name: String) {
        match tag {
            Tag::Tag1 => self.tag1 = name,
            Tag::Tag2 => self.tag2 = name,
            Tag::Tag3 => self.tag3 = name,
            Tag::Tag4 => self.tag4 = name,
            Tag::Tag5 => self.tag5 = name,
            Tag::Tag6 => self.tag6 = name,
            Tag::Tag7 => self.tag7 = name,
            Tag::Tag8 => self.tag8 = name,
        }
    }

    pub fn get(&self, tag: &Tag) -> &str {
        match tag {
            Tag::Tag1 => &self.tag1,
            Tag::Tag2 => &self.tag2,
            Tag::Tag3 => &self.tag3,
            Tag::Tag4 => &self.tag4,
            Tag::Tag5 => &self.tag5,
            Tag::Tag6 => &self.tag6,
            Tag::Tag7 => &self.tag7,
            Tag::Tag8 => &self.tag8,
        }
    }
}

pub fn tag_badge_color(tag: &Tag) -> iced::Color {
    match *tag {
        Tag::Tag1 => TAG_COLORS.red,
        Tag::Tag2 => TAG_COLORS.green,
        Tag::Tag3 => TAG_COLORS.yellow,
        Tag::Tag4 => TAG_COLORS.blue,
        Tag::Tag5 => TAG_COLORS.purple,
        Tag::Tag6 => TAG_COLORS.orange,
        Tag::Tag7 => TAG_COLORS.gray,
        Tag::Tag8 => TAG_COLORS.cyan,
    }
}

pub fn keybind_char_to_tag(c: &str) -> Option<Tag> {
    match c {
        "a" => Some(Tag::Tag1),
        "o" => Some(Tag::Tag2),
        "e" => Some(Tag::Tag3),
        "u" => Some(Tag::Tag4),
        _ => None,
    }
}

fn user_pressed_previous_image(model: &mut crate::Model) -> Effect {
    let preload_path = model.pathlist.step_left(&model.config);
    match preload_path {
        Some(path) => Effect::PreloadImages(vec![path], model.canvas_dimensions.unwrap()),
        None => Effect::None,
    }
}

fn user_pressed_next_image(model: &mut crate::Model) -> Effect {
    let preload_path = model.pathlist.step_right(&model.config);
    match preload_path {
        Some(path) => Effect::PreloadImages(vec![path], model.canvas_dimensions.unwrap()),
        None => Effect::None,
    }
}

fn tag_and_move_on(model: &mut crate::Model, tag: Tag) -> Effect {
    if model.pathlist.paths.is_empty() {
        return Effect::None;
    }

    model.pathlist.current_mut().metadata.tag = Some(tag);
    user_pressed_next_image(model)
}

fn view_image<'a>(
    image: &'a ImageInfo,
    tag_names: &TagNames,
    dim: Option<Dim>,
    highlight: bool,
    is_main_image: bool,
) -> Element<'a, Message> {
    let name_and_color = image.metadata.tag.as_ref().map(|tag| {
        let name = tag_names.get(tag);
        let color = tag_badge_color(tag);
        (name.to_owned(), color)
    });
    match &image.data {
        PreloadImage::Loaded(LoadedImageAndThumb { image, thumb }) => {
            if dim.is_some() {
                // TODO: bad way to figure out that it's a thumbnail
                view_loaded_image(Some(thumb), name_and_color, dim, highlight, is_main_image)
            } else {
                view_loaded_image(Some(image), name_and_color, dim, highlight, is_main_image)
            }
        }
        PreloadImage::Loading(_) | PreloadImage::NotLoading => {
            view_loaded_image(None, name_and_color, dim, highlight, is_main_image)
        }
    }
}

fn view_loaded_image(
    image: Option<&ImageData>,
    name_and_color: Option<(String, iced::Color)>,
    dim: Option<Dim>,
    highlight: bool,
    send_resize_messages: bool,
) -> Element<Message> {
    let pixel_canvas = PixelCanvas::new(image, send_resize_messages);
    let (w, h) = match dim {
        Some(dim) => (
            Length::Fixed(dim.width as f32),
            Length::Fixed(dim.height as f32),
        ),
        None => (Length::Fill, Length::Fill),
    };
    let canvas_widget = canvas(pixel_canvas).width(w).height(h);

    let image_with_border = if highlight {
        widget::container(canvas_widget)
            .style(|_: &iced::Theme| {
                widget::container::Style::default().border(iced::Border {
                    radius: iced::border::radius(5),
                    color: Color::from_rgb(0.0, 0.2, 0.8),
                    width: 3.0,
                })
            })
            .padding(3)
    } else {
        widget::container(canvas_widget)
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

fn preload_list_status_string_pathlist(
    pathlist: &PathList,
    task_manager: &crate::task_manager::TaskManager,
) -> String {
    let mut s = String::new();
    let total = pathlist.paths.len();
    let counts = pathlist.get_counts();
    let loaded = counts.loaded;
    let loading = counts.loading;
    let not_loading = counts.not_loading;

    // Get task manager information
    let (ls_dir_tasks, preload_tasks) = task_manager.get_task_counts();

    s.push_str(&format!("Loaded: {loaded}/{total}"));
    if loading > 0 {
        s.push_str(&format!(", Loading: {loading}"));
    }
    if not_loading > 0 {
        s.push_str(&format!(", Not loading: {not_loading}"));
    }
    if preload_tasks > 0 {
        s.push_str(&format!(", In flight: {preload_tasks}"));
    }
    if ls_dir_tasks > 0 {
        s.push_str(&format!(", Dir loading: {ls_dir_tasks}"));
    }
    s
}

fn view_tag_button_row<'a>(
    editing_tag_name: Option<&(Tag, String, iced::widget::text_input::Id)>,
    expanded: Option<Tag>,
    names: &'a TagNames,
    nums: &HashMap<Tag, u32>,
) -> Element<'a, Message> {
    let tag_button_helper = |name: String, tag: &Tag, button_style: ButtonStyle| {
        let num = *nums.get(tag).unwrap_or(&0);
        view_tag_button(
            name,
            tag,
            num,
            button_style.basic,
            button_style.hover,
            button_style.press,
            expanded == Some(*tag),
            match editing_tag_name {
                Some((t, name, id)) if *t == *tag => Some((name.clone(), id.clone())),
                _ => None,
            },
        )
    };

    column![
        row![
            tag_button_helper(names.tag1.clone(), &Tag::Tag1, ui::RED_BUTTON_STYLE),
            tag_button_helper(names.tag2.clone(), &Tag::Tag2, ui::GREEN_BUTTON_STYLE),
            tag_button_helper(names.tag3.clone(), &Tag::Tag3, ui::YELLOW_BUTTON_STYLE),
            tag_button_helper(names.tag4.clone(), &Tag::Tag4, ui::BLUE_BUTTON_STYLE),
        ],
        row![
            tag_button_helper(names.tag5.clone(), &Tag::Tag5, ui::PURPLE_BUTTON_STYLE),
            tag_button_helper(names.tag6.clone(), &Tag::Tag6, ui::ORANGE_BUTTON_STYLE),
            tag_button_helper(names.tag7.clone(), &Tag::Tag7, ui::GRAY_BUTTON_STYLE),
            tag_button_helper(names.tag8.clone(), &Tag::Tag8, ui::CYAN_BUTTON_STYLE),
        ]
    ]
    .into()
}

fn view_tag_button<'a>(
    text: String,
    tag: &Tag,
    num: u32,
    basic_bg: Color,
    hover_bg: Color,
    press_bg: Color,
    expanded: bool,
    editing_tag_name: Option<(String, widget::text_input::Id)>,
) -> Element<'a, Message> {
    let style = iced::widget::button::Style {
        background: Some(iced::Background::Color(basic_bg)),
        text_color: iced::Color::from_rgb(1.0, 1.0, 1.0),
        border: iced::Border::default(),
        shadow: iced::Shadow::default(),
    };
    let style_hovered = style.with_background(iced::Background::Color(hover_bg));

    let style_pressed = style.with_background(iced::Background::Color(press_bg));

    let button_height = 33;
    let tag_button = widget::Button::new(widget::text!("{text} ({num})"))
        .style(move |_, status| match &status {
            widget::button::Status::Active => style,
            widget::button::Status::Hovered => style_hovered,
            widget::button::Status::Pressed => style_pressed,
            widget::button::Status::Disabled => style,
        })
        .on_press(Message::Sorting(SortingMessage::UserPressedTagButton(*tag)))
        .width(Length::Fill)
        .height(button_height);

    let more_button = widget::button("...")
        .style(move |_, status| match &status {
            widget::button::Status::Active => style,
            widget::button::Status::Hovered => style_hovered,
            widget::button::Status::Pressed => style_pressed,
            widget::button::Status::Disabled => style,
        })
        .on_press(Message::Sorting(SortingMessage::UserPressedRenameTag(*tag)))
        .width(45)
        .height(button_height);

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

    let rename_input: Option<Element<Message>> = editing_tag_name.map(|(text, id)| {
        widget::text_input("tag name", &text)
            .on_input(|text| Message::Sorting(SortingMessage::UserEditTagName(text)))
            .on_submit(Message::Sorting(SortingMessage::UserPressedSubmitRenameTag))
            .id(id.clone())
            .into()
    });

    match rename_input {
        Some(widget) => widget,
        None => row![tag_button, drop_down_button].into(),
    }
}

fn tag_dropdown_button(text: &str, message: SortingMessage) -> Element<Message> {
    button(text)
        .on_press(Message::Sorting(message))
        .width(250)
        .into()
}

// Public functions for flattened sorting model
pub fn update_sorting_model(
    model: &mut crate::Model,
    message: SortingMessage,
    config: &crate::Config,
) -> crate::Effect {
    log::info!("Keyboard event, in sorting model");
    match message {
        SortingMessage::UserPressedPreviousImage => user_pressed_previous_image(model),
        SortingMessage::UserPressedNextImage => user_pressed_next_image(model),
        SortingMessage::ImagePreloaded(path, image, thumb) => {
            if let Some(path) = model
                .pathlist
                .image_preload_complete(&path, image, thumb, config)
            {
                crate::Effect::PreloadImages(vec![path], model.canvas_dimensions.unwrap())
            } else {
                crate::Effect::None
            }
        }
        SortingMessage::KeyboardEvent(iced::keyboard::Event::KeyPressed { key, .. })
            if key == iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape) =>
        {
            log::info!("Pressed escape, clearing edit tag name");
            model.editing_tag_name = None;
            Effect::None
        }
        SortingMessage::KeyboardEvent(_) if is_typing_action(model) => crate::Effect::None,
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
                    tag_and_move_on(model, tag)
                }
                iced::keyboard::Key::Named(iced::keyboard::key::Named::Delete) => {
                    tag_and_move_on(model, Tag::Tag7)
                }
                iced::keyboard::Key::Named(iced::keyboard::key::Named::Backspace) => {
                    if !model.pathlist.paths.is_empty() {
                        model.pathlist.paths[model.pathlist.index].metadata.tag = None;
                    }
                    crate::Effect::None
                }
                _ => crate::Effect::None,
            },
            _ => crate::Effect::None,
        },
        SortingMessage::UserPressedTagButton(tag) => {
            tag_and_move_on(model, tag);
            crate::Effect::None
        }
        SortingMessage::UserPressedRenameTag(tag) => {
            let id = widget::text_input::Id::unique();
            model.editing_tag_name = Some((tag, "".to_owned(), id.clone()));
            model.expanded_dropdown = None;
            crate::Effect::FocusElement(id)
        }
        SortingMessage::UserPressedSubmitRenameTag => {
            let (tag, new_tag_name, _) = model.editing_tag_name.take().unwrap();
            model.tag_names.update(tag, new_tag_name);
            crate::Effect::None
        }
        SortingMessage::UserPressedCancelRenameTag => {
            model.editing_tag_name = None;
            crate::Effect::None
        }
        SortingMessage::UserEditTagName(text) => {
            model.editing_tag_name.as_mut().unwrap().1 = text;
            crate::Effect::None
        }
        SortingMessage::UserPressedMoveTag(tag) => {
            model.expanded_dropdown = None;
            crate::Effect::MoveThenLs(tag)
        }
        SortingMessage::UserPressedTagMenu(maybe_tag) => {
            if model.expanded_dropdown.as_ref() == maybe_tag.as_ref() {
                model.expanded_dropdown = None;
            } else {
                model.expanded_dropdown = maybe_tag;
            }
            crate::Effect::None
        }
        SortingMessage::CanvasResized(dim) => {
            println!("Canvas resized to: {}x{}", dim.width, dim.height);
            if model.canvas_dimensions.as_ref() != Some(&dim) {
                model.canvas_dimensions = Some(dim);
                // Start the preloading now
                crate::Effect::LsDir
            } else {
                crate::Effect::None
            }
        }
    }
}

pub fn view_sorting_model<'a>(
    model: &'a crate::Model,
    config: &'a crate::Config,
    task_manager: &'a crate::task_manager::TaskManager,
) -> iced::Element<'a, crate::Message> {
    // Check if pathlist is empty to avoid panics
    if model.pathlist.paths.is_empty() {
        return widget::text("No images found").into();
    }

    let main_image_view = view_image_with_thumbs(config.thumbnail_style.clone(), model);

    let preload_status_string = preload_list_status_string_pathlist(&model.pathlist, task_manager);
    debug!("Preload status: {preload_status_string}");

    let mut tag_count = std::collections::HashMap::new();

    for metadata in model.pathlist.paths.iter().map(|info| &info.metadata) {
        if let Some(tag) = metadata.tag {
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
        model.editing_tag_name.as_ref(),
        model.expanded_dropdown,
        &model.tag_names,
        &tag_count,
    );

    let action_buttons = row![
        widget::button(widget::text!("{}", t!("<- Previous")))
            .on_press(crate::Message::Sorting(
                SortingMessage::UserPressedPreviousImage
            ))
            .padding(10),
        widget::button(widget::text!("{}", t!("Next ->")))
            .on_press(crate::Message::Sorting(
                SortingMessage::UserPressedNextImage
            ))
            .padding(10),
        widget::button(widget::text!("{}", t!("Select Folder")))
            .on_press(crate::Message::UserPressedSelectFolder)
            .padding(10),
    ];

    let content = column![
        main_image_view,
        status_text,
        tag_buttons,
        action_buttons,
        widget::text(preload_status_string),
    ];

    center(content).into()
}

fn is_typing_action(model: &crate::Model) -> bool {
    model.editing_tag_name.is_some()
}

fn view_image_with_thumbs<'a>(
    sorting_view_style: SortingViewStyle,
    model: &'a crate::Model,
) -> Element<'a, Message> {
    match sorting_view_style {
        SortingViewStyle::NoThumbnails => view_with_no_thumbnails(model),
        SortingViewStyle::ThumbsAbove => view_with_thumbnails_on_top(model),
    }
}

fn view_with_no_thumbnails(model: &crate::Model) -> Element<Message> {
    let image = view_image(
        model.pathlist.current(),
        &model.tag_names,
        None,
        false,
        true,
    );

    image
}

fn view_with_thumbnails_on_top(model: &crate::Model) -> Element<Message> {
    let image = view_image(
        model.pathlist.current(),
        &model.tag_names,
        None,
        false,
        true,
    );

    // Three on each side
    let num_thumbs = 3;
    let mut thumbs = Vec::new();
    let from = model.pathlist.index.saturating_sub(num_thumbs);
    let to = min(
        model.pathlist.index + num_thumbs,
        model.pathlist.paths.len() - 1,
    );
    for i in from..=to {
        let img = &model.pathlist.paths[i];
        let highlight = i == model.pathlist.index;
        let thumb = view_image(
            img,
            &model.tag_names,
            Some(model.config.thumbnail_size),
            highlight,
            false,
        );
        thumbs.push(thumb);
    }

    column![widget::Row::from_vec(thumbs), image].into()
}
