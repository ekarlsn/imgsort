use iced::widget::{button, column, row, text, text_input};
use iced::Element;
use std::collections::HashMap;

use crate::{Config, Effect, Message};

#[derive(Debug, Clone)]
pub struct SettingsModel {
    pub fields: HashMap<SettingsFieldName, (String, String)>,
}

#[derive(Debug, Clone)]
pub enum SettingsMessage {
    UserUpdatedField(SettingsFieldName, String),
    Save,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum SettingsFieldName {
    PreloadBackNum,
    PreloadFrontNum,
    ScaleDownSizeWidth,
    ScaleDownSizeHeight,
    Tag1Shortcut,
}

impl SettingsModel {
    pub fn new(config: &Config) -> Self {
        Self {
            fields: HashMap::from_iter([
                (
                    SettingsFieldName::PreloadBackNum,
                    (config.preload_back_num.to_string(), String::from("")),
                ),
                (
                    SettingsFieldName::PreloadFrontNum,
                    (config.preload_front_num.to_string(), String::from("")),
                ),
                (
                    SettingsFieldName::ScaleDownSizeWidth,
                    (config.scale_down_size.0.to_string(), String::from("")),
                ),
                (
                    SettingsFieldName::ScaleDownSizeHeight,
                    (config.scale_down_size.1.to_string(), String::from("")),
                ),
                (
                    SettingsFieldName::Tag1Shortcut,
                    ("a".to_owned(), String::from("")),
                ),
            ]),
        }
    }

    pub fn update(&mut self, message: SettingsMessage, config: &mut Config) -> Effect {
        match message {
            SettingsMessage::UserUpdatedField(field, text) => {
                self.fields.insert(field, (text, "".to_owned()));
                Effect::None
            }
            SettingsMessage::Save => {
                let (text, error) = self
                    .fields
                    .get_mut(&SettingsFieldName::PreloadBackNum)
                    .unwrap();
                match text.parse() {
                    Ok(num) => config.preload_back_num = num,
                    Err(_) => *error = "Invalid number".to_owned(),
                }
                let (text, error) = self
                    .fields
                    .get_mut(&SettingsFieldName::PreloadFrontNum)
                    .unwrap();
                match text.parse() {
                    Ok(num) => config.preload_front_num = num,
                    Err(_) => *error = "Invalid number".to_owned(),
                }
                let (text, error) = self
                    .fields
                    .get_mut(&SettingsFieldName::ScaleDownSizeWidth)
                    .unwrap();
                match text.parse() {
                    Ok(num) => config.scale_down_size.0 = num,
                    Err(_) => *error = "Invalid number".to_owned(),
                }
                let (text, error) = self
                    .fields
                    .get_mut(&SettingsFieldName::ScaleDownSizeHeight)
                    .unwrap();
                match text.parse() {
                    Ok(num) => config.scale_down_size.1 = num,
                    Err(_) => *error = "Invalid number".to_owned(),
                }
                Effect::None
            }
        }
    }

    pub fn view(&self) -> Element<Message> {
        let (preload_back_text, preload_back_error) =
            self.fields.get(&SettingsFieldName::PreloadBackNum).unwrap();
        let (preload_front_text, preload_front_error) = self
            .fields
            .get(&SettingsFieldName::PreloadFrontNum)
            .unwrap();
        let (scale_down_width_text, scale_down_width_error) = self
            .fields
            .get(&SettingsFieldName::ScaleDownSizeWidth)
            .unwrap();
        let (scale_down_height_text, scale_down_height_error) = self
            .fields
            .get(&SettingsFieldName::ScaleDownSizeHeight)
            .unwrap();
        let (tag1_text, tag1_error) = self.fields.get(&SettingsFieldName::Tag1Shortcut).unwrap();

        column![
            text("Settings"),
            row![
                text("Preload back"),
                text_input("Preload back", preload_back_text)
                    .id("preload_back_num")
                    .on_input(|text| Message::Settings(SettingsMessage::UserUpdatedField(
                        SettingsFieldName::PreloadBackNum,
                        text
                    ))),
                text(preload_back_error)
            ],
            row![
                text("Preload front"),
                text_input("Preload front", preload_front_text)
                    .id("preload_front_num")
                    .on_input(|text| Message::Settings(SettingsMessage::UserUpdatedField(
                        SettingsFieldName::PreloadFrontNum,
                        text
                    ))),
                text(preload_front_error),
            ],
            row![
                text("Scale down size WxH"),
                text_input("Width", scale_down_width_text)
                    .id("scale_down_size_width")
                    .on_input(|text| Message::Settings(SettingsMessage::UserUpdatedField(
                        SettingsFieldName::ScaleDownSizeWidth,
                        text
                    ))),
                text(scale_down_width_error),
                text_input("Height", scale_down_height_text)
                    .id("scale_down_size_height")
                    .on_input(|text| Message::Settings(SettingsMessage::UserUpdatedField(
                        SettingsFieldName::ScaleDownSizeHeight,
                        text
                    ))),
                text(scale_down_height_error),
            ],
            text("Shortcuts"),
            row![
                text("Tag 1"),
                text_input("Tag 1", tag1_text)
                    .id("tag_1_shortcut")
                    .on_input(|text| Message::Settings(SettingsMessage::UserUpdatedField(
                        SettingsFieldName::Tag1Shortcut,
                        text
                    ))),
                text(tag1_error),
            ],
            button("Save").on_press(Message::Settings(SettingsMessage::Save)),
        ]
        .into()
    }
}
