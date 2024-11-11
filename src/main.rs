use iced::futures;
use iced::widget::{self, center, column, image, row, text};
use iced::{Center, Element, Fill, Right, Task};

pub fn main() -> iced::Result {
    iced::application(Model::title, Model::update, Model::view).run_with(Model::new)
}

#[derive(Debug)]
struct Model {
    image_state: ImageState,
    current_image: image::Handle,
    current_index: usize,
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

#[derive(Debug)]
enum ImageState {
    Loading,
    Loaded { pokemon: Pokemon },
    Errored,
}

#[derive(Debug, Clone)]
enum Message {
    PokemonFound(Result<Pokemon, Error>),
    Search,
}

impl PathList {
    fn new(paths: Vec<String>) -> Self {
        let paths = paths
            .iter()
            .map(|path| (path.clone(), Metadata { tags: vec![] }))
            .collect();
        Self { paths, index: 0 }
    }
}

impl PreloadList {
    fn new(preload_back_num: usize, preload_front_num: usize) -> Self {
        let mut images = Vec::new();
        for _ in 0..preload_back_num {
            images.push(PreloadImage::OutOfRange);
        }
        for _ in 0..(1 + preload_front_num) {
            images.push(PreloadImage::Loading);
        }
        Self {
            images,
            preload_back_num,
            preload_front_num,
            index: 0,
        }
    }
}

#[derive(Debug)]
enum PreloadImage {
    Loading,
    Loaded(image::Handle),
    Errored,
    OutOfRange,
}

const IMAGE_PATHS: [&str; 3] = ["pictures/one.jpg", "pictures/two.jpg", "pictures/three.jpg"];

impl Model {
    fn new() -> (Self, Task<Message>) {
        let img: image::Handle = image::Handle::from_path("pictures/one.jpg");
        let paths = get_files_in_folder("pictures").unwrap();
        (
            Self {
                image_state: ImageState::Loading,
                current_image: img,
                current_index: 0,
                pathlist: PathList::new(paths),
                preload_list: PreloadList::new(2, 3),
            },
            Self::search(),
        )
    }

    fn search() -> Task<Message> {
        Task::perform(Pokemon::search(), Message::PokemonFound)
    }

    fn title(&self) -> String {
        format!("ImageViewer")
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::PokemonFound(Ok(pokemon)) => {
                self.image_state = ImageState::Loaded { pokemon };

                Task::none()
            }
            Message::PokemonFound(Err(_error)) => {
                self.image_state = ImageState::Errored;

                Task::none()
            }
            Message::Search => {
                self.current_index = (self.current_index + 1) % IMAGE_PATHS.len();
                self.current_image = image::Handle::from_path(IMAGE_PATHS[self.current_index]);

                Task::none()
            }
        }
    }

    fn view(&self) -> Element<Message> {
        let content = column![
            image::viewer(self.current_image.clone()),
            button("Keep searching!").on_press(Message::Search),
        ];
        center(content).into()
    }
}

#[derive(Debug, Clone)]
struct Pokemon {
    number: u16,
    name: String,
    description: String,
    image: image::Handle,
}

impl Pokemon {
    const TOTAL: u16 = 807;

    fn view(&self, img: &image::Handle) -> Element<Message> {
        row![
            image::viewer(img.clone()),
            column![
                row![
                    text(&self.name).size(30).width(Fill),
                    text!("#{}", self.number).size(20).color([0.5, 0.5, 0.5]),
                ]
                .align_y(Center)
                .spacing(20),
                self.description.as_ref(),
            ]
            .spacing(20),
        ]
        .spacing(20)
        .align_y(Center)
        .into()
    }

    async fn search() -> Result<Pokemon, Error> {
        use rand::Rng;
        use serde::Deserialize;

        #[derive(Debug, Deserialize)]
        struct Entry {
            name: String,
            flavor_text_entries: Vec<FlavorText>,
        }

        #[derive(Debug, Deserialize)]
        struct FlavorText {
            flavor_text: String,
            language: Language,
        }

        #[derive(Debug, Deserialize)]
        struct Language {
            name: String,
        }

        let id = {
            let mut rng = rand::rngs::OsRng;

            rng.gen_range(0..Pokemon::TOTAL)
        };

        let fetch_entry = async {
            let url = format!("https://pokeapi.co/api/v2/pokemon-species/{id}");

            reqwest::get(&url).await?.json().await
        };

        let (entry, image): (Entry, _) =
            futures::future::try_join(fetch_entry, Self::fetch_image(id)).await?;

        let description = entry
            .flavor_text_entries
            .iter()
            .find(|text| text.language.name == "en")
            .ok_or(Error::LanguageError)?;

        Ok(Pokemon {
            number: id,
            name: entry.name.to_uppercase(),
            description: description
                .flavor_text
                .chars()
                .map(|c| if c.is_control() { ' ' } else { c })
                .collect(),
            image,
        })
    }

    async fn fetch_image(id: u16) -> Result<image::Handle, reqwest::Error> {
        let url = format!(
            "https://raw.githubusercontent.com/PokeAPI/sprites/master/sprites/pokemon/{id}.png"
        );

        #[cfg(not(target_arch = "wasm32"))]
        {
            let bytes = reqwest::get(&url).await?.bytes().await?;

            Ok(image::Handle::from_bytes(bytes))
        }

        #[cfg(target_arch = "wasm32")]
        Ok(image::Handle::from_path(url))
    }
}

#[derive(Debug, Clone)]
enum Error {
    APIError,
    LanguageError,
}

impl From<reqwest::Error> for Error {
    fn from(error: reqwest::Error) -> Error {
        dbg!(error);

        Error::APIError
    }
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
                    file_names.push(file_name_str.to_string());
                }
            }
        }
    }

    Ok(file_names)
}
