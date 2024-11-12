use anyhow::Result;
use iced::widget::{self, center, column, row, text};
use iced::{Element, Task};
use iced_wgpu::graphics::image::image_rs::EncodableLayout;
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
    config: Config,
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
    tags: Vec<String>,
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
    UserPressedNextImage,
    UserPressedPreviousImage,
    ImagePreloaded(String, ImageData),
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

    fn image_loaded(&mut self, offset_index: isize, image: ImageData) {
        if offset_index >= -(self.preload_back_num as isize)
            && offset_index <= (self.preload_front_num as isize)
        {
            let index = (self.index as isize + offset_index) % (self.images.len() as isize);
            let index: usize = index.try_into().unwrap(); // TODO, better error handling
            self.images[index] = PreloadImage::Loaded(image);
        }
    }
}

#[derive(Debug)]
enum PreloadImage {
    Loading(String),
    Loaded(ImageData),
    Errored,
    OutOfRange,
}

enum Effect {
    LsDir(String),
}

impl Model {
    fn new() -> (Self, Task<Message>) {
        let config = Config {
            preload_back_num: 2,
            preload_front_num: 3,
            scale_down_size: (800, 600),
        };
        let paths = get_files_in_folder("pictures/real").unwrap();
        debug!("Paths: {:?}", paths);
        let (preload_list, preload_tasks) = PreloadList::new(
            config.preload_back_num,
            config.preload_front_num,
            paths.clone(),
        );
        let action = initial_preloads(preload_tasks, config.clone());
        (
            Self {
                pathlist: PathList::new(paths.clone()),
                preload_list,
                config,
            },
            action,
        )
    }

    fn title(&self) -> String {
        format!("ImageViewer")
    }

    fn update_with_effect(&mut self, message: Message) -> Task<Message> {
        match self.update(message) {
            Effect::LsDir(path) => {}
        }

        Task::none()
    }

    fn update_with_effect(&mut self, message: Message) -> Effect {
        debug!("Message: {:?}", message);
        let task = match message {
            Message::UserPressedPreviousImage => {
                if self.pathlist.index > 0 {
                    self.pathlist.index = (self.pathlist.index - 1) % self.pathlist.paths.len();
                    self.preload_list.index = (self.preload_list.index as isize - 1)
                        .rem_euclid(self.preload_list.images.len() as isize)
                        .try_into()
                        .unwrap();

                    let preload_index =
                        self.pathlist.index as isize - self.preload_list.preload_back_num as isize;
                    if preload_index >= 0 {
                        let new_preload_image =
                            self.pathlist.paths[preload_index as usize].0.clone();
                        self.preload_list.images
                            [self.preload_list.index - self.preload_list.preload_back_num] =
                            PreloadImage::Loading(new_preload_image.clone());

                        preload_image_task(new_preload_image, self.config.clone())
                    } else {
                        Task::none()
                    }
                } else {
                    Task::none()
                }
            }
            Message::UserPressedNextImage => {
                if self.pathlist.index + 1 < self.pathlist.paths.len() {
                    self.pathlist.index = (self.pathlist.index + 1) % self.pathlist.paths.len();
                    self.preload_list.index =
                        (self.preload_list.index + 1) % self.preload_list.images.len();
                    let preload_index =
                        self.pathlist.index as isize - self.preload_list.preload_back_num as isize;
                    if preload_index < self.pathlist.paths.len() as isize {
                        let new_preload_image =
                            self.pathlist.paths[preload_index as usize].0.clone();
                        self.preload_list.images
                            [self.preload_list.index + self.preload_list.preload_front_num] =
                            PreloadImage::Loading(new_preload_image.clone());

                        preload_image_task(new_preload_image, self.config.clone())
                    } else {
                        Task::none()
                    }
                } else {
                    Task::none()
                }
            }
            Message::ImagePreloadFailed(path) => Task::none(),
            Message::ImagePreloaded(path, image) => {
                if let Some(offset_index) = self.pathlist.get_offset_index(&path) {
                    debug!("Offset index: {offset_index:?}");
                    self.preload_list.image_loaded(offset_index, image);
                }

                Task::none()
            }
        };
        debug!("Preload list: {:?}", self.preload_list);
        task
    }

    fn view(&self) -> Element<Message> {
        let content2: Element<_> = match self.preload_list.current_image() {
            PreloadImage::Loaded(image) => column![
                view_image(&image),
                text(format!(
                    "Image {index}/{total}",
                    index = self.pathlist.index + 1,
                    total = self.pathlist.paths.len()
                )),
                text(format!(
                    "Path: {path}",
                    path = self.pathlist.paths[self.pathlist.index].0
                ))
            ]
            .into(),
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

fn initial_preloads(paths: Vec<String>, config: Config) -> Task<Message> {
    let tasks: Vec<Task<Message>> = paths
        .into_iter()
        .map(|path| {
            let path2 = path.clone();
            let config2 = config.clone();
            preload_image_task(path2, config2)
        })
        .collect();
    Task::batch(tasks)
}

fn preload_image_task(path: String, config: Config) -> Task<Message> {
    let fut = tokio::task::spawn_blocking(move || preload_image(path, config));
    Task::perform(fut, |res| match res {
        Ok((path4, image)) => Message::ImagePreloaded(path4, image),
        Err(_) => Message::ImagePreloadFailed("too hard to know".to_owned()),
    })
}

fn preload_image(path: String, config: Config) -> (String, ImageData) {
    let image = ImageReader::open(path.as_str())
        .unwrap()
        .decode()
        .unwrap()
        .resize(
            config.scale_down_size.0,
            config.scale_down_size.1,
            image::imageops::FilterType::Lanczos3,
        )
        .to_rgba8();
    let width = image.width();
    let height = image.height();
    let image = ImageData {
        data: image.to_vec(),
        width,
        height,
    };
    // let handle = widget::image::Handle::from_path(&path);
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
