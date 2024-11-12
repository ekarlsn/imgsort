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
    iced::application(Model::title, Model::update_with_task, Model::view)
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
    SortingModel(SortingModel),
}

#[derive(Debug)]
struct SortingModel {
    pathlist: PathList,
    preload_list: PreloadList,
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
    Noop,
    UserPressedNextImage,
    UserPressedPreviousImage,
    ImagePreloaded(String, ImageData),
    ImagePreloadFailed(String),
    ListDirCompleetd(Vec<String>),
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

#[derive(Debug, Clone, Eq, PartialEq)]
enum Effect {
    None,
    LsDir(String),
    PreloadImages(Vec<String>),
}

impl Model {
    fn new() -> (Self, Effect) {
        (
            Self {
                config: Config {
                    preload_back_num: 1,
                    preload_front_num: 2,
                    scale_down_size: (800, 600),
                },
                state: ModelState::LoadingListDir,
            },
            Effect::LsDir("pictures/real".to_owned()),
        )
    }

    fn new_with_task() -> (Self, Task<Message>) {
        let (new_self, effect) = Self::new();
        let task = effect_to_task(effect, new_self.config.clone());
        (new_self, task)
    }

    fn go_to_sorting_model(&mut self, paths: Vec<String>) -> Effect {
        debug!("Going to sorting model");
        let (preload_list, preload_tasks) = PreloadList::new(
            self.config.preload_back_num,
            self.config.preload_front_num,
            paths.clone(),
        );
        let action = initial_preloads(preload_tasks);
        self.state = ModelState::SortingModel(SortingModel {
            pathlist: PathList::new(paths.clone()),
            preload_list,
        });
        action
    }

    fn title(&self) -> String {
        format!("ImageViewer")
    }

    fn update_with_task(&mut self, message: Message) -> Task<Message> {
        effect_to_task(self.update(message), self.config.clone())
    }

    fn update(&mut self, message: Message) -> Effect {
        debug!("Message: {:?}", message);
        let effect = match &mut self.state {
            ModelState::SortingModel(model) => Model::update_sorting_model(model, message),
            ModelState::LoadingListDir => self.update_loading_model(message),
        };
        debug!("Effect: {:?}", effect);
        effect
    }

    fn update_loading_model(&mut self, message: Message) -> Effect {
        match message {
            Message::ListDirCompleetd(paths) => self.go_to_sorting_model(paths),
            _ => Effect::None,
        }
    }

    fn update_sorting_model(model: &mut SortingModel, message: Message) -> Effect {
        match message {
            Message::UserPressedPreviousImage => user_pressed_previous_image(model),
            Message::UserPressedNextImage => user_pressed_next_image(model),
            Message::ImagePreloadFailed(path) => Effect::None,
            Message::ImagePreloaded(path, image) => {
                if let Some(offset_index) = model.pathlist.get_offset_index(&path) {
                    debug!("Offset index: {offset_index:?}");
                    model.preload_list.image_loaded(offset_index, image);
                }

                Effect::None
            }
            Message::Noop => todo!(),
            Message::ListDirCompleetd(vec) => todo!(),
        }
    }

    fn view(&self) -> Element<Message> {
        match &self.state {
            ModelState::SortingModel(model) => Model::view_sorting_model(model),
            ModelState::LoadingListDir => text("Loading...").into(),
        }
    }

    fn view_sorting_model(model: &SortingModel) -> Element<Message> {
        let content2: Element<_> = match model.preload_list.current_image() {
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

fn effect_to_task(effect: Effect, config: Config) -> Task<Message> {
    match effect {
        Effect::None => Task::none(),
        Effect::LsDir(path) => ls_dir_task(path),
        Effect::PreloadImages(paths) => preload_images_task(paths, config),
    }
}

fn initial_preloads(paths: Vec<String>) -> Effect {
    Effect::PreloadImages(paths)
}

fn ls_dir_task(path: String) -> Task<Message> {
    Task::perform(get_files_in_folder_async(path), |res| match res {
        Ok(paths) => Message::ListDirCompleetd(paths),
        Err(_) => Message::Noop,
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
            Ok((path4, image)) => Message::ImagePreloaded(path4, image),
            Err(_) => Message::ImagePreloadFailed("too hard to know".to_owned()),
        }))
    }
    Task::batch(tasks)
}

fn preload_image_effect(path: String) -> Effect {
    Effect::PreloadImages(vec![path])
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
                ModelState::SortingModel(sorting_model) => {
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

        let (mut model, effect) = Model::new();
        assert_eq!(effect, Effect::LsDir("pictures/real".to_owned()));

        let effect = model.update(Message::ListDirCompleetd(vec![
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

        let effect = model.update(Message::ImagePreloaded(
            "pictures/real/2.jpg".to_owned(),
            ImageData {
                data: vec![1, 2, 3],
                width: 1,
                height: 1,
            },
        ));

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
            model.update(Message::UserPressedNextImage),
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

        assert_eq!(
            model.update(Message::UserPressedPreviousImage),
            Effect::None,
        );

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
        assert_eq!(
            model.update(Message::UserPressedPreviousImage),
            Effect::None,
        );
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
            model.update(Message::UserPressedNextImage),
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
            model.update(Message::UserPressedNextImage),
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
            model.update(Message::UserPressedNextImage),
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

        // Hit the right end
        assert_eq!(model.update(Message::UserPressedNextImage), Effect::None,);
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
