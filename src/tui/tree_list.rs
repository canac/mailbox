use super::navigable_list::{Keyed, NavigableList};
use tui::widgets::ListState;

pub trait Depth {
    fn get_depth(&self) -> usize;
}

pub struct TreeList<Item: Depth> {
    state: ListState,
    items: Vec<Item>,
}

impl<Item> NavigableList<Item> for TreeList<Item>
where
    Item: Depth + Keyed,
{
    fn get_items(&self) -> &Vec<Item> {
        &self.items
    }

    fn get_items_mut(&mut self) -> &mut Vec<Item> {
        &mut self.items
    }

    fn get_cursor(&self) -> Option<usize> {
        self.state.selected()
    }

    fn set_cursor(&mut self, new_cursor: Option<usize>) {
        self.state.select(new_cursor);
    }
}

impl<Item: Depth + Keyed> TreeList<Item> {
    // Create a new list with no items
    pub fn new() -> Self {
        TreeList {
            state: ListState::default(),
            items: vec![],
        }
    }

    // Return a reference to the list state
    pub fn get_list_state(&mut self) -> &mut ListState {
        &mut self.state
    }

    // Move the cursor to the next sibling or ancestor in the tree, skipping over descendants
    pub fn next_sibling(&mut self) {
        match self.get_cursor_item().map(|item| item.get_depth()) {
            None => self.next(),
            Some(current_depth) => {
                let new_index = self
                    .items
                    .iter()
                    .enumerate()
                    .skip(self.get_cursor().unwrap_or(0) + 1)
                    .find(|(_, item)| item.get_depth() <= current_depth)
                    .map(|(index, _)| index);
                if new_index.is_some() {
                    self.set_cursor(new_index)
                }
            }
        }
    }

    // Move the cursor to the previous sibling or ancestor in the tree, skipping over descendants
    pub fn previous_sibling(&mut self) {
        match self.get_cursor_item().map(|item| item.get_depth()) {
            None => self.previous(),
            Some(current_depth) => {
                let new_index = self
                    .items
                    .iter()
                    .enumerate()
                    .take(self.get_cursor().unwrap_or(0))
                    .rfind(|(_, item)| item.get_depth() <= current_depth)
                    .map(|(index, _)| index);
                if new_index.is_some() {
                    self.set_cursor(new_index)
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Item {
        id: u64,
        depth: usize,
    }

    impl Depth for Item {
        fn get_depth(&self) -> usize {
            self.depth
        }
    }

    impl Keyed for Item {
        fn get_key(&self) -> u64 {
            self.id
        }
    }

    impl Item {
        fn new(id: u64, depth: usize) -> Self {
            Self { id, depth }
        }
    }

    fn get_tree() -> TreeList<Item> {
        let mut tree = TreeList::new();
        tree.replace_items(vec![
            Item::new(0, 0),
            Item::new(1, 0),
            Item::new(2, 1),
            Item::new(3, 2),
            Item::new(4, 1),
            Item::new(5, 2),
            Item::new(6, 2),
            Item::new(7, 0),
            Item::new(8, 1),
        ]);
        tree
    }

    #[test]
    fn test_next_sibling() {
        let mut tree = get_tree();
        tree.set_cursor(Some(2));

        tree.next_sibling();
        assert_eq!(tree.get_cursor(), Some(4));

        tree.next_sibling();
        assert_eq!(tree.get_cursor(), Some(7));
    }

    #[test]
    fn test_next_sibling_no_cursor() {
        let mut tree = get_tree();
        tree.next_sibling();
        assert_eq!(tree.get_cursor(), Some(0));
    }

    #[test]
    fn test_next_sibling_end() {
        let mut tree = get_tree();
        tree.set_cursor(Some(7));
        tree.next_sibling();
        assert_eq!(tree.get_cursor(), Some(7));
    }

    #[test]
    fn test_previous_sibling() {
        let mut tree = get_tree();
        tree.set_cursor(Some(5));

        tree.previous_sibling();
        assert_eq!(tree.get_cursor(), Some(4));

        tree.previous_sibling();
        assert_eq!(tree.get_cursor(), Some(2));

        tree.previous_sibling();
        assert_eq!(tree.get_cursor(), Some(1));

        tree.previous_sibling();
        assert_eq!(tree.get_cursor(), Some(0));
    }

    #[test]
    fn test_previous_sibling_no_cursor() {
        let mut tree = get_tree();
        tree.previous_sibling();
        assert_eq!(tree.get_cursor(), Some(0));
    }

    #[test]
    fn test_previous_sibling_beginning() {
        let mut tree = get_tree();
        tree.set_cursor(Some(0));
        tree.previous_sibling();
        assert_eq!(tree.get_cursor(), Some(0));
    }
}
