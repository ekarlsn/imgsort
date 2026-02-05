use std::collections::HashMap;

use iced::widget::{self, button, column, container, row, text};
use iced::{Color, Element};

use rust_i18n::t;

use crate::sorting::tag_badge_color;
use crate::{Message, Tag, TagNames};

pub fn view_actions_tab(
    selected_action_tag: &Option<Tag>,
    tag_names: TagNames,
    tag_counts: &HashMap<Tag, u32>,
) -> Element<'static, Message> {
    if let Some(tag) = selected_action_tag {
        // Show tag action view
        let tag_name = tag_names.get(tag).to_string();

        container(
            column![
                row![
                    button(text(t!("← Back"))).on_press(Message::UserPressedActionBack),
                    text(tag_name).size(24),
                ]
                .spacing(10)
                .align_y(iced::Alignment::Center),
                column![
                    button(text(t!("Delete"))).width(200),
                    button(text(t!("Move")))
                        .width(200)
                        .on_press(Message::UserPressedActionCopy(*tag)),
                    button(text(t!("Copy"))).width(200),
                ]
                .spacing(10)
                .padding(20),
            ]
            .spacing(20),
        )
        .padding(20)
        .into()
    } else {
        // Show tag button list
        let mut buttons = Vec::new();

        for (tag, name) in tag_names.enumerate() {
            if let Some(count) = tag_counts.get(&tag) {
                buttons.push(view_action_tag_button(tag, name.clone(), *count));
            }
        }

        let buttons_col = column(buttons).spacing(10);

        let tag_buttons = column![
            text(t!("Actions")).size(24),
            text(t!("Select a tag to perform actions:")).size(16),
            buttons_col,
        ]
        .spacing(15);

        container(tag_buttons).padding(20).into()
    }
}

fn view_action_tag_button(tag: Tag, name: String, count: u32) -> Element<'static, Message> {
    let tag_name = format!("{name} ({count})");

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
