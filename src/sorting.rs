use iced::widget::{self, button, canvas, center, column, row, stack};
use iced::{Color, Element, Length};
use iced_aw::{drop_down, DropDown};
use itertools::Itertools;
use rust_i18n::t;
use std::collections::HashMap;

use crate::image_widget::PixelCanvas;
use crate::{Config, Effect, ImageData, ImageInfo, Message, PathList, PreloadImage};

// Constants
pub const TAGGING_CHARS: &str = "aoeupy";

// Tag constants
pub const TAG1: Tag = Tag::Tag1;
pub const TAG2: Tag = Tag::Tag2;
pub const TAG3: Tag = Tag::Tag3;
pub const TAG4: Tag = Tag::Tag4;
pub const TAG5: Tag = Tag::Tag5;

#[derive(Debug)]
pub struct SortingModel {
    pub pathlist: PathList,
    pub expanded_dropdown: Option<Tag>,
    pub editing_tag_name: Option<(Tag, String, widget::text_input::Id)>,
    pub tag_names: TagNames,
}

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
    ImagePreloaded(String, ImageData),
    ImagePreloadFailed(String),
    KeyboardEvent(iced::keyboard::Event),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Copy)]
pub enum Tag {
    Tag1,
    Tag2,
    Tag3,
    Tag4,
    Tag5,
}

#[derive(Debug, Clone)]
pub struct TagNames {
    pub tag1: String,
    pub tag2: String,
    pub tag3: String,
    pub tag4: String,
}

pub enum SortingViewStyle {
    Thumbnails,
    #[allow(unused)]
    BeforeAfter,
}

#[derive(Clone, Debug)]
pub struct Dim {
    pub width: u32,
    pub height: u32,
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

impl TagNames {
    pub fn new() -> Self {
        Self {
            tag1: String::from("Red"),
            tag2: String::from("Green"),
            tag3: String::from("Yellow"),
            tag4: String::from("Blue"),
        }
    }

    pub fn update(&mut self, tag: Tag, name: String) {
        match tag {
            Tag::Tag1 => self.tag1 = name,
            Tag::Tag2 => self.tag2 = name,
            Tag::Tag3 => self.tag3 = name,
            Tag::Tag4 => self.tag4 = name,
            Tag::Tag5 => (),
        }
    }

    pub fn get(&self, tag: &Tag) -> &str {
        match tag {
            Tag::Tag1 => &self.tag1,
            Tag::Tag2 => &self.tag2,
            Tag::Tag3 => &self.tag3,
            Tag::Tag4 => &self.tag4,
            Tag::Tag5 => "",
        }
    }
}

pub fn tag_badge_color(tag: &Tag) -> iced::Color {
    match *tag {
        TAG1 => TAG_COLORS.red,
        TAG2 => TAG_COLORS.green,
        TAG3 => TAG_COLORS.yellow,
        TAG4 => TAG_COLORS.blue,
        _ => TAG_COLORS.other,
    }
}

pub fn keybind_char_to_tag(c: &str) -> Option<Tag> {
    match c {
        "a" => Some(TAG1),
        "o" => Some(TAG2),
        "e" => Some(TAG3),
        "u" => Some(TAG4),
        _ => None,
    }
}

fn is_typing_action(model: &SortingModel) -> bool {
    model.editing_tag_name.is_some()
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
            return Effect::PreloadImages(vec![info.path.clone()]);
        }
    }

    Effect::None
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
            return Effect::PreloadImages(vec![info.path.clone()]);
        }
    }

    Effect::None
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
            return Effect::PreloadImages(vec![e.path.clone()]);
        }
    }
    Effect::None
}

fn tag_and_move_on(model: &mut SortingModel, tag: Tag) -> Effect {
    model.pathlist.current_mut().metadata.tag = Some(tag);
    user_pressed_next_image(model)
}

impl SortingModel {
    pub fn update(&mut self, message: SortingMessage, config: &Config) -> Effect {
        match message {
            SortingMessage::UserPressedPreviousImage => user_pressed_previous_image(self),
            SortingMessage::UserPressedNextImage => user_pressed_next_image(self),
            SortingMessage::ImagePreloadFailed(_path) => Effect::None,
            SortingMessage::ImagePreloaded(path, image) => {
                self.pathlist
                    .paths
                    .iter_mut()
                    .find(|info| info.path == path)
                    .map(|info| {
                        info.data = PreloadImage::Loaded(image);
                        info
                    });

                schedule_next_preload_image_after_one_finished(&self.pathlist, config)
            }
            SortingMessage::KeyboardEvent(_) if is_typing_action(self) => Effect::None,
            SortingMessage::KeyboardEvent(event) => match event {
                iced::keyboard::Event::KeyPressed { key, modifiers, .. } => match key.as_ref() {
                    iced::keyboard::Key::Character("h")
                    | iced::keyboard::Key::Named(iced::keyboard::key::Named::ArrowLeft) => {
                        user_pressed_previous_image(self)
                    }
                    iced::keyboard::Key::Character("t" | "l")
                    | iced::keyboard::Key::Named(iced::keyboard::key::Named::ArrowRight) => {
                        user_pressed_next_image(self)
                    }
                    iced::keyboard::Key::Character(c)
                        if !modifiers.control() && TAGGING_CHARS.contains(c) =>
                    {
                        let tag = keybind_char_to_tag(c).unwrap();
                        // Any tagging character
                        tag_and_move_on(self, tag)
                    }
                    iced::keyboard::Key::Named(iced::keyboard::key::Named::Delete) => {
                        tag_and_move_on(self, TAG5)
                    }
                    iced::keyboard::Key::Named(iced::keyboard::key::Named::Backspace) => {
                        self.pathlist.paths[self.pathlist.index].metadata.tag = None;
                        Effect::None
                    }
                    _ => Effect::None,
                },
                _ => Effect::None,
            },
            SortingMessage::UserPressedTagButton(tag) => {
                tag_and_move_on(self, tag);
                Effect::None
            }
            SortingMessage::UserPressedRenameTag(tag) => {
                let id = widget::text_input::Id::unique();
                self.editing_tag_name = Some((tag, "".to_owned(), id.clone()));
                self.expanded_dropdown = None;
                Effect::FocusElement(id)
            }
            SortingMessage::UserPressedSubmitRenameTag => {
                let (tag, new_tag_name, _) = self.editing_tag_name.take().unwrap();
                self.tag_names.update(tag, new_tag_name);
                Effect::None
            }
            SortingMessage::UserPressedCancelRenameTag => {
                self.editing_tag_name = None;
                Effect::None
            }
            SortingMessage::UserEditTagName(text) => {
                self.editing_tag_name.as_mut().unwrap().1 = text;

                Effect::None
            }
            SortingMessage::UserPressedMoveTag(tag) => {
                self.expanded_dropdown = None;
                Effect::MoveImagesWithTag(tag)
            }
            SortingMessage::UserPressedTagMenu(maybe_tag) => {
                if self.expanded_dropdown.as_ref() == maybe_tag.as_ref() {
                    self.expanded_dropdown = None;
                } else {
                    self.expanded_dropdown = maybe_tag;
                }
                Effect::None
            }
        }
    }

    pub fn view<'a>(&'a self, config: &'a Config) -> Element<'a, Message> {
        let sorting_view_style = SortingViewStyle::Thumbnails;

        let main_image_view = view_image_with_thumbs(sorting_view_style, self, config);

        let preload_status_string = preload_list_status_string_pathlist(&self.pathlist);

        let mut tag_count = HashMap::new();

        for metadata in self.pathlist.paths.iter().map(|info| &info.metadata) {
            if let Some(tag) = metadata.tag {
                let count = tag_count.entry(tag).or_insert(0);
                *count += 1;
            }
        }

        let status_text = widget::text(format!(
            "({index}/{total}) {path}",
            index = self.pathlist.index + 1,
            total = self.pathlist.paths.len(),
            path = self.pathlist.current().path,
        ));

        let tag_buttons = view_tag_button_row(self.expanded_dropdown, &self.tag_names, &tag_count);

        let action_buttons = row![
            widget::button(widget::text!("{}", t!("<- Previous")))
                .on_press(Message::Sorting(SortingMessage::UserPressedPreviousImage))
                .padding(10),
            widget::button(widget::text!("{}", t!("Next ->")))
                .on_press(Message::Sorting(SortingMessage::UserPressedNextImage))
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

        let popup = self
            .editing_tag_name
            .as_ref()
            .map(|(_, text, id)| view_rename_tag_modal(text.as_str(), id.clone()));

        stack![content].push_maybe(popup).into()
    }
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

fn placeholder_text<'a>(msg: impl AsRef<str> + 'a, dim: &Dim) -> widget::Text<'a> {
    widget::text(msg.as_ref().to_owned())
        .width(dim.width as f32)
        .height(dim.height as f32)
}

fn view_image<'a>(
    image: &'a ImageInfo,
    tag_names: &TagNames,
    dim: Dim,
    highlight: bool,
) -> Element<'a, Message> {
    let name_and_color = image.metadata.tag.as_ref().map(|tag| {
        let name = tag_names.get(tag);
        let color = tag_badge_color(tag);
        (name.to_owned(), color)
    });
    match &image.data {
        PreloadImage::Loaded(image) => view_loaded_image(image, name_and_color, dim, highlight),
        PreloadImage::Loading(path) => placeholder_text(format!("Loading {path}..."), &dim).into(),
        PreloadImage::OutOfRange => placeholder_text("Out of range", &dim).into(),
    }
}

fn view_loaded_image(
    image: &ImageData,
    name_and_color: Option<(String, iced::Color)>,
    dim: Dim,
    highlight: bool,
) -> Element<Message> {
    let pixel_canvas = PixelCanvas::new(image.clone());
    let canvas_widget = canvas(pixel_canvas)
        .width(dim.width as f32)
        .height(dim.height as f32);

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

fn preload_list_status_string_pathlist(pathlist: &PathList) -> String {
    let mut s = String::new();
    let total = pathlist.paths.len();
    let loaded = pathlist
        .paths
        .iter()
        .filter(|info| matches!(info.data, PreloadImage::Loaded(_)))
        .count();
    let loading = pathlist
        .paths
        .iter()
        .filter(|info| matches!(info.data, PreloadImage::Loading(_)))
        .count();
    s.push_str(&format!("Loaded: {loaded}/{total}"));
    if loading > 0 {
        s.push_str(&format!(", Loading: {loading}"));
    }
    s
}

fn view_tag_button_row<'a>(
    expanded: Option<Tag>,
    names: &'a TagNames,
    nums: &HashMap<Tag, u32>,
) -> Element<'a, Message> {
    let red = names.tag1.as_str();
    let green = names.tag2.as_str();
    let yellow = names.tag3.as_str();
    let blue = names.tag4.as_str();
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
        .on_press(Message::Sorting(SortingMessage::UserPressedTagButton(*tag)))
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
            *tag,
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
