use anyhow::Result;
use iced::widget::{self, center, column, row, text};
use iced::{Element, Task};
use image::ImageReader;
use log::debug;

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
    iced::application(Model::title, Model::update, Model::view).run_with(Model::new)
}

#[derive(Debug)]
struct Model {
    pathlist: PathList,
    preload_list: PreloadList,
}

#[derive(Debug)]
struct PathList {
    paths: Vec<(String, Metadata)>,
    index: usize,
}

#[derive(Debug)]
struct Metadata {
    tags: Vec<String>,
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
    UserPressedNextImage,
    UserPressedPreviousImage,
    ImagePreloaded(String, widget::image::Handle),
    ImagePreloadFailed(String),
}

impl PathList {
    fn new(paths: Vec<String>) -> Self {
        let paths = paths
            .iter()
            .map(|path| (path.clone(), Metadata { tags: vec![] }))
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

    fn image_loaded(&mut self, offset_index: isize, handle: widget::image::Handle) {
        if offset_index >= -(self.preload_back_num as isize)
            && offset_index <= (self.preload_front_num as isize)
        {
            let index = (self.index as isize + offset_index) % (self.images.len() as isize);
            let index: usize = index.try_into().unwrap(); // TODO, better error handling
            self.images[index] = PreloadImage::Loaded(handle);
        }
    }
}

#[derive(Debug)]
enum PreloadImage {
    Loading(String),
    Loaded(widget::image::Handle),
    Errored,
    OutOfRange,
}

impl Model {
    fn new() -> (Self, Task<Message>) {
        let paths = get_files_in_folder("pictures").unwrap();
        debug!("Paths: {:?}", paths);
        let (preload_list, preload_tasks) = PreloadList::new(2, 3, paths.clone());
        let action = initial_preloads(preload_tasks);
        (
            Self {
                pathlist: PathList::new(paths.clone()),
                preload_list,
            },
            action,
        )
    }

    fn title(&self) -> String {
        format!("ImageViewer")
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        debug!("Message: {:?}", message);
        let task = match message {
            Message::UserPressedPreviousImage => {
                if self.pathlist.index > 0 {
                    self.pathlist.index = (self.pathlist.index - 1) % self.pathlist.paths.len();
                    self.preload_list.index =
                        (self.preload_list.index - 1) % self.preload_list.images.len();
                }

                Task::none()
            }
            Message::UserPressedNextImage => {
                self.pathlist.index = (self.pathlist.index + 1) % self.pathlist.paths.len();
                self.preload_list.index =
                    (self.preload_list.index + 1) % self.preload_list.images.len();

                Task::none()
            }
            Message::ImagePreloadFailed(path) => Task::none(),
            Message::ImagePreloaded(path, handle) => {
                if let Some(offset_index) = self.pathlist.get_offset_index(&path) {
                    debug!("Offset index: {offset_index:?}");
                    self.preload_list.image_loaded(offset_index, handle);
                }

                Task::none()
            }
        };
        debug!("Model: {:?}", self.preload_list);
        task
    }

    fn view(&self) -> Element<Message> {
        let content2: Element<_> = match self.preload_list.current_image() {
            PreloadImage::Loaded(handle) => view_image(&handle),
            PreloadImage::Loading(path) => text(format!("Loading {path}...")).into(),
            PreloadImage::Errored => text("Error loading image").into(),
            PreloadImage::OutOfRange => text("Out of range").into(),
        };
        let content2 = column![
            content2,
            row![
                button("Previous image").on_press(Message::UserPressedPreviousImage),
                button("Next image").on_press(Message::UserPressedNextImage),
            ]
        ];

        center(content2).into()
    }
}

fn initial_preloads(paths: Vec<String>) -> Task<Message> {
    let tasks: Vec<Task<Message>> = paths
        .into_iter()
        .map(|path| {
            let path2 = path.clone();
            let fut = tokio::task::spawn_blocking(move || preload_image(path2));
            Task::perform(fut, |res| match res {
                Ok((path4, handle)) => Message::ImagePreloaded(path4, handle),
                Err(_) => Message::ImagePreloadFailed("too hard to know".to_owned()),
            })
        })
        .collect();
    Task::batch(tasks)
}

fn preload_image(path: String) -> (String, widget::image::Handle) {
    let handle = widget::image::Handle::from_path(&path);
    (path, handle)
}

fn view_image(handle: &iced::widget::image::Handle) -> Element<Message> {
    iced::widget::image::viewer(handle.clone()).into()
}

fn button(text: &str) -> widget::Button<'_, Message> {
    widget::button(text).padding(10)
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
                    file_names.push(format!("{folder_path}/{file_name_str}"));
                }
            }
        }
    }

    file_names.sort();
    Ok(file_names)
}

#[cfg(test)]
mod tests {
    use super::*;

    enum PreloadExpect {
        Loading(String),
        Loaded,
        OutOfRange,
    }

    fn expect_preloads(actual: &Vec<PreloadImage>, expected: Vec<PreloadExpect>) {
        assert_eq!(actual.len(), expected.len());
        for i in 0..actual.len() {
            match &actual[i] {
                PreloadImage::Loading(path) => match &expected[i] {
                    PreloadExpect::Loading(expected_path) => {
                        assert_eq!(path, expected_path);
                    }
                    _ => panic!("Expected loading"),
                },
                PreloadImage::Loaded(_handle) => match &expected[i] {
                    PreloadExpect::Loaded => {}
                    _ => panic!("Expected loaded"),
                },
                PreloadImage::OutOfRange => match &expected[i] {
                    PreloadExpect::OutOfRange => {}
                    _ => panic!("Expected out of range"),
                },
                _ => panic!("Unexpected preload state"),
            }
        }
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
        let (mut model, tasks) = Model::new();
        let _ = model.update(Message::ImagePreloaded(
            "pictures/1.jpg".to_owned(),
            widget::image::Handle::from_path("pictures/1.jpg"),
        ));

        expect_preloads(
            &model.preload_list.images,
            vec![
                PreloadExpect::OutOfRange,
                PreloadExpect::OutOfRange,
                PreloadExpect::Loaded,
                PreloadExpect::Loading("pictures/2.jpg".to_owned()),
                PreloadExpect::Loading("pictures/3.jpg".to_owned()),
                PreloadExpect::Loading("pictures/4.jpg".to_owned()),
            ],
        );

        let _ = model.update(Message::ImagePreloaded(
            "pictures/3.jpg".to_owned(),
            widget::image::Handle::from_path("pictures/3.jpg"),
        ));

        expect_preloads(
            &model.preload_list.images,
            vec![
                PreloadExpect::OutOfRange,
                PreloadExpect::OutOfRange,
                PreloadExpect::Loaded,
                PreloadExpect::Loading("pictures/2.jpg".to_owned()),
                PreloadExpect::Loaded,
                PreloadExpect::Loading("pictures/4.jpg".to_owned()),
            ],
        );

        let _ = model.update(Message::UserPressedNextImage);
        expect_preloads(
            &model.preload_list.images,
            vec![
                // PreloadExpect::Loading("pictures/ferris.png".to_owned()),
                // TODO this should be Loading ferris
                PreloadExpect::OutOfRange,
                PreloadExpect::OutOfRange,
                PreloadExpect::Loaded,
                PreloadExpect::Loading("pictures/2.jpg".to_owned()),
                PreloadExpect::Loaded,
                PreloadExpect::Loading("pictures/4.jpg".to_owned()),
            ],
        );
    }
}
