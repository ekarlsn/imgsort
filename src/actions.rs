use iced::widget::{self, button, column, container, row, text};
use iced::{Color, Element};

use crate::sorting::tag_badge_color;
use crate::{Message, Tag, TagNames};

pub fn view_actions_tab(
    selected_action_tag: &Option<Tag>,
    tag_names: &TagNames,
) -> Element<'static, Message> {
    if let Some(tag) = selected_action_tag {
        // Show tag action view
        let tag_name = tag_names.get(tag).to_string();

        container(
            column![
                row![
                    button("← Back").on_press(Message::UserPressedActionBack),
                    text(tag_name).size(24),
                ]
                .spacing(10)
                .align_y(iced::Alignment::Center),
                column![
                    button("Delete").width(200),
                    button("Move").width(200),
                    button("Copy")
                        .width(200)
                        .on_press(Message::UserPressedActionCopy(tag.clone())),
                ]
                .spacing(10)
                .padding(20),
            ]
            .spacing(20),
        )
        .padding(20)
        .into()
    } else {
        // Show tag list
        let tag_buttons = column![
            text("Actions").size(24),
            text("Select a tag to perform actions:").size(16),
            column![
                view_action_tag_button(Tag::Tag1, tag_names.tag1.to_string()),
                view_action_tag_button(Tag::Tag2, tag_names.tag2.to_string()),
                view_action_tag_button(Tag::Tag3, tag_names.tag3.to_string()),
                view_action_tag_button(Tag::Tag4, tag_names.tag4.to_string()),
            ]
            .spacing(10),
        ]
        .spacing(15);

        container(tag_buttons).padding(20).into()
    }
}

fn view_action_tag_button(tag: Tag, name: String) -> Element<'static, Message> {
    let tag_name = name;

    widget::button(text(tag_name))
        .width(200)
        .style(move |_theme, _status| {
            let color = tag_badge_color(&tag);
            widget::button::Style {
                background: Some(iced::Background::Color(color)),
                text_color: Color::WHITE,
                border: iced::Border {
                    color,
                    width: 1.0,
                    radius: 4.0.into(),
                },
                shadow: iced::Shadow::default(),
            }
        })
        .on_press(Message::UserPressedActionTag(tag))
        .into()
}
