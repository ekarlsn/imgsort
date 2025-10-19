use std::cmp::min;

use crate::{
    sorting::Tag, Config, ImageData, ImageInfo, Metadata, PreloadImage, PRELOAD_IN_FLIGHT,
};
use itertools::Itertools;
use log::debug;

#[derive(Debug)]
pub struct PathList {
    pub paths: Vec<ImageInfo>,
    pub index: usize,
    pub preload_back_num: usize,
    pub preload_front_num: usize,
}

impl PathList {
    pub fn new(paths: Vec<String>, preload_back_num: usize, preload_front_num: usize) -> Self {
        let paths = paths
            .iter()
            .map(|path| ImageInfo {
                path: path.clone(),
                data: PreloadImage::NotLoading,
                metadata: Metadata { tag: None },
            })
            .collect();
        Self {
            paths,
            index: 0,
            preload_back_num,
            preload_front_num,
        }
    }

    // Preload order?
    // cache-size = 100, how many picture are kept in the list, when you scroll past preload limit
    // back = 10, how many you start preloading backwards
    // front = 30, how many you start preloading forwards
    // in_flight = 8 (Or number of cores?), how many you preload at the same time
    pub fn get_initial_preload_images(&mut self) -> Vec<String> {
        let from = self
            .index
            .saturating_sub(std::cmp::min(self.preload_back_num, PRELOAD_IN_FLIGHT / 2));
        let to = *[
            self.index + self.preload_front_num + 1,
            self.paths.len(),
            from + PRELOAD_IN_FLIGHT,
        ]
        .iter()
        .min()
        .expect("The iter is not empty");

        let mut paths = Vec::new();
        for i in from..to {
            let p = self.paths[i].path.clone();
            debug!("Setting loading state for index {i}");
            self.paths[i].data = PreloadImage::Loading(p.clone());
            paths.push(p);
        }
        paths
    }

    pub fn step_right(&mut self, config: &Config) -> Option<String> {
        // Check if pathlist is empty
        if self.paths.is_empty() {
            return None;
        }

        // We're already at the far right
        if self.index == self.paths.len() - 1 {
            return None;
        }

        self.index += 1;

        // Check if we've already filled the preload cache size
        if self
            .paths
            .iter()
            .filter(|image: &&ImageInfo| is_loading(*image))
            .count()
            >= PRELOAD_IN_FLIGHT
        {
            return None;
        }

        self.preload_next_right(config)
    }

    fn preload_next_right(&mut self, config: &Config) -> Option<String> {
        let max_preload_index = min(
            self.index + config.preload_front_num + 1,
            self.paths.len() - 1,
        );
        debug!("Preloading next right image, up to {max_preload_index}");
        for i in self.index..max_preload_index {
            let e = &mut self.paths[i];
            if is_not_loading(e) {
                let p = e.path.clone();
                e.data = PreloadImage::Loading(p.clone());
                return Some(p);
            }
        }

        None
    }

    pub fn get_counts(&self) -> ImageStateCounts {
        ImageStateCounts {
            loaded: self.paths.iter().filter(|image| is_loaded(image)).count(),
            loading: self.paths.iter().filter(|image| is_loading(image)).count(),
            not_loading: self
                .paths
                .iter()
                .filter(|image| is_not_loading(image))
                .count(),
        }
    }

    pub fn image_preload_complete(
        &mut self,
        path: &str,
        image: ImageData,
        config: &Config,
    ) -> Option<String> {
        if let Some(index) = self.paths.iter().position(|info| info.path == path) {
            self.paths[index].data = PreloadImage::Loaded(image);
        }

        schedule_next_preload_image_after_one_finished(self, config)
    }

    pub fn tag_of(&self, path: &str) -> Option<Tag> {
        self.paths
            .iter()
            .find(|info| info.path == path)
            .and_then(|info| info.metadata.tag)
    }

    pub fn prev(&self) -> Option<&ImageInfo> {
        if self.index == 0 {
            None
        } else {
            Some(&self.paths[self.index - 1])
        }
    }

    pub fn current(&self) -> &ImageInfo {
        &self.paths[self.index]
    }

    pub fn next(&self) -> Option<&ImageInfo> {
        self.paths.get(self.index + 1)
    }

    pub fn current_mut(&mut self) -> &mut ImageInfo {
        &mut self.paths[self.index]
    }
}

fn schedule_next_preload_image_after_one_finished(
    pathlist: &mut PathList,
    config: &Config,
) -> Option<String> {
    // Don't need to check in-flight num here, since one is just completed, leaving a space
    let curr = pathlist.index;

    let forward = pathlist.paths.iter().enumerate().skip(curr);
    let rev = pathlist
        .paths
        .iter()
        .enumerate()
        .rev()
        .skip(pathlist.paths.len() - curr);

    let mut should_preload = None;
    for (i, e) in forward.interleave(rev) {
        if is_not_loading(e)
            && i <= curr + config.preload_front_num
            && i >= curr - min(config.preload_back_num, curr)
        {
            debug!("Setting loading state for index {i}");
            should_preload = Some((i, e.path.clone()));
            break;
        }
    }
    match should_preload {
        Some((i, path)) => {
            pathlist.paths[i].data = PreloadImage::Loading(path.clone());
            Some(path)
        }
        None => None,
    }
}

fn is_loading(image: &ImageInfo) -> bool {
    match image.data {
        PreloadImage::Loading(_) => true,
        _ => false,
    }
}

#[allow(dead_code)] // For symmetry
fn is_loaded(image: &ImageInfo) -> bool {
    match image.data {
        PreloadImage::Loaded(_) => true,
        _ => false,
    }
}

fn is_not_loading(image: &ImageInfo) -> bool {
    match image.data {
        PreloadImage::NotLoading => true,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sorting::Tag;

    fn create_test_pathlist(paths: Vec<&str>, back: usize, front: usize) -> PathList {
        PathList::new(
            paths.into_iter().map(|s| s.to_string()).collect(),
            back,
            front,
        )
    }

    fn create_test_config() -> Config {
        Config {
            preload_back_num: 10,
            preload_front_num: 30,
            scale_down_size: (800, 100),
        }
    }

    #[test]
    fn test_current_prev_next() {
        let mut pathlist = create_test_pathlist(vec!["img1.jpg", "img2.jpg", "img3.jpg"], 1, 2);

        // At index 0
        assert_eq!(pathlist.current().path, "img1.jpg");
        assert!(pathlist.prev().is_none());
        assert_eq!(pathlist.next().unwrap().path, "img2.jpg");

        // Move to index 1
        pathlist.index = 1;
        assert_eq!(pathlist.current().path, "img2.jpg");
        assert_eq!(pathlist.prev().unwrap().path, "img1.jpg");
        assert_eq!(pathlist.next().unwrap().path, "img3.jpg");

        // Move to last index
        pathlist.index = 2;
        assert_eq!(pathlist.current().path, "img3.jpg");
        assert_eq!(pathlist.prev().unwrap().path, "img2.jpg");
        assert!(pathlist.next().is_none());
    }

    #[test]
    fn test_get_initial_preload_images_small_list() {
        let mut pathlist = create_test_pathlist(vec!["img1.jpg", "img2.jpg", "img3.jpg"], 2, 5);
        let preload = pathlist.get_initial_preload_images();

        // With small list, should preload all images
        assert_eq!(preload.len(), 3);
        assert_eq!(preload, vec!["img1.jpg", "img2.jpg", "img3.jpg"]);
    }

    #[test]
    fn test_get_list_preloads_finish() {
        let paths: Vec<String> = (0..80).map(|i| format!("img{}.jpg", i)).collect();
        let mut pathlist = PathList::new(paths, 3, 7);
        let preload = pathlist.get_initial_preload_images();

        // Should be limited by PRELOAD_IN_FLIGHT (8)
        assert_eq!(preload.len(), 8);
        assert_eq!(preload[0], "img0.jpg");
        assert_eq!(preload[7], "img7.jpg");

        let config = create_test_config();
        // Nothing gets scheduled, because too many in flight already
        let next_preload = schedule_next_preload_image_after_one_finished(&mut pathlist, &config);
        assert_eq!(next_preload.unwrap(), "img8.jpg");
    }

    #[test]
    fn test_get_initial_preload_images_large_list() {
        let paths: Vec<String> = (0..20).map(|i| format!("img{}.jpg", i)).collect();
        let mut pathlist = PathList::new(paths, 3, 7);
        let preload = pathlist.get_initial_preload_images();

        // Should be limited by PRELOAD_IN_FLIGHT (8)
        assert_eq!(preload.len(), 8);
        assert_eq!(preload[0], "img0.jpg");
        assert_eq!(preload[7], "img7.jpg");
    }

    #[test]
    fn test_get_initial_preload_images_middle_index() {
        let paths: Vec<String> = (0..20).map(|i| format!("img{}.jpg", i)).collect();
        let mut pathlist = PathList::new(paths, 2, 5);
        pathlist.index = 10;

        let preload = pathlist.get_initial_preload_images();

        // Should include some behind (limited by PRELOAD_IN_FLIGHT/2 = 4) and ahead
        assert_eq!(preload.len(), 8);
        // From index 8 to 15 (8 images total)
        assert_eq!(preload[0], "img8.jpg");
        assert_eq!(preload[7], "img15.jpg");
    }

    #[test]
    fn test_tag_of() {
        let mut pathlist = create_test_pathlist(vec!["img1.jpg", "img2.jpg", "img3.jpg"], 1, 2);

        // Initially no tags
        assert_eq!(pathlist.tag_of("img1.jpg"), None);
        assert_eq!(pathlist.tag_of("img2.jpg"), None);
        assert_eq!(pathlist.tag_of("nonexistent.jpg"), None);

        // Set a tag
        pathlist.paths[1].metadata.tag = Some(Tag::Tag2);
        assert_eq!(pathlist.tag_of("img2.jpg"), Some(Tag::Tag2));
        assert_eq!(pathlist.tag_of("img1.jpg"), None);
    }

    #[test]
    fn test_schedule_next_preload_image_after_one_finished() {
        let mut pathlist =
            create_test_pathlist(vec!["img1.jpg", "img2.jpg", "img3.jpg", "img4.jpg"], 1, 2);
        pathlist.index = 1; // Start at img2.jpg

        // Should return img2.jpg (current)
        let config = create_test_config();
        let next = schedule_next_preload_image_after_one_finished(&mut pathlist, &config);
        assert_eq!(next, Some("img2.jpg".to_string()));

        // Mark img1 as NotLoading
        pathlist.paths[0].data = PreloadImage::NotLoading;

        // Should return next in interleaved order (img1.jpg is next in interleave: forward[img3,img4], rev[img1])
        let next = schedule_next_preload_image_after_one_finished(&mut pathlist, &config);
        assert_eq!(next, Some("img1.jpg".to_string()));

        // Mark img3 as NotLoading
        pathlist.paths[2].data = PreloadImage::NotLoading;

        // Should return img3.jpg (next forward in interleaved order)
        let next = schedule_next_preload_image_after_one_finished(&mut pathlist, &config);
        assert_eq!(next, Some("img3.jpg".to_string()));
    }

    #[test]
    fn test_schedule_next_preload_no_loading_images() {
        let mut pathlist = create_test_pathlist(vec!["img1.jpg", "img2.jpg", "img3.jpg"], 1, 2);
        pathlist.index = 1; // Start at img2.jpg

        let config = create_test_config();
        let next = schedule_next_preload_image_after_one_finished(&mut pathlist, &config);
        assert_eq!(next, Some("img2.jpg".to_string()));
    }
}

pub struct ImageStateCounts {
    pub loaded: usize,
    pub loading: usize,
    pub not_loading: usize,
}
