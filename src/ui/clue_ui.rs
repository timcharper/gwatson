use gtk::prelude::*;
use gtk::{Box, Frame, Grid, Label, Orientation};
use std::cell::RefCell;
use std::rc::Rc;

use crate::model::LayoutConfiguration;
use crate::model::{Clue, CluesSizing};
use crate::model::{ClueOrientation, TileAssertion};
use crate::model::{ClueType, HorizontalClueType, VerticalClueType};
use crate::ui::clue_tile_ui::ClueTileUI;
use crate::ui::ResourceSet;

#[derive(Debug)]
struct ClueTooltipData {
    clue: Clue,
    resources: Rc<ResourceSet>,
}

#[derive(Debug)]
enum TemplateElement {
    Label(String),
    Tile(usize),
}

const NEW_GROUP_CSS_CLASS: &str = "new-group";

pub struct ClueUI {
    pub frame: Frame,
    pub cells: Vec<ClueTileUI>,
    pub orientation: ClueOrientation,
    tooltip_data: Rc<RefCell<Option<ClueTooltipData>>>,
    tooltip_widget: Rc<RefCell<Option<gtk::Box>>>,
    resources: Rc<ResourceSet>,
    layout: CluesSizing,
}

impl ClueUI {
    fn parse_template_elements(&self, template: &str) -> Vec<TemplateElement> {
        let mut elements = Vec::new();
        let mut current_text = String::new();
        let mut chars = template.chars().peekable();

        while let Some(c) = chars.next() {
            if c == '{' {
                // If we have accumulated text, add it as a label
                if !current_text.is_empty() {
                    elements.push(TemplateElement::Label(current_text.clone()));
                    current_text.clear();
                }

                // Parse the token
                let mut token = String::new();
                while let Some(&next_c) = chars.peek() {
                    chars.next();
                    if next_c == '}' {
                        break;
                    }
                    token.push(next_c);
                }

                // Handle tile tokens
                if let Ok(tile_idx) = token.trim_start_matches('t').parse::<usize>() {
                    elements.push(TemplateElement::Tile(tile_idx));
                }
            } else {
                current_text.push(c);
            }
        }

        // Add any remaining text
        if !current_text.is_empty() {
            elements.push(TemplateElement::Label(current_text));
        }

        elements
    }

    fn parse_template(&self, template: &str, clue_data: &ClueTooltipData) -> gtk::Box {
        let box_container = gtk::Box::new(gtk::Orientation::Horizontal, 5);

        // Transform TemplateElements into GTK widgets
        self.parse_template_elements(template)
            .into_iter()
            .flat_map(|element| match element {
                TemplateElement::Label(text) => {
                    let label = Label::new(None);
                    label.set_markup(&text);
                    label.set_wrap(true);
                    label.set_max_width_chars(40);
                    Some(label.upcast::<gtk::Widget>())
                }
                TemplateElement::Tile(tile_idx) => {
                    // Get the tile assertion and create an image if it exists
                    self.cells
                        .get(tile_idx)
                        .and_then(|_| clue_data.clue.assertions.get(tile_idx))
                        .and_then(|ta| self.resources.get_tile_icon(&ta.tile))
                        .map(|pixbuf| {
                            let image = gtk::Image::from_pixbuf(Some(&pixbuf));
                            image.upcast::<gtk::Widget>()
                        })
                }
            })
            .for_each(|widget| box_container.append(&widget));

        box_container
    }

    fn create_tooltip_widget(&self) -> gtk::Box {
        let tooltip_box = Box::new(Orientation::Vertical, 5);
        let clue_data = self.tooltip_data.borrow();
        if clue_data.is_none() {
            return tooltip_box;
        }

        let clue_data = clue_data.as_ref().unwrap();

        tooltip_box.set_margin_start(5);
        tooltip_box.set_margin_end(5);
        tooltip_box.set_margin_top(5);
        tooltip_box.set_margin_bottom(5);

        // Add title
        let title_box = Box::new(Orientation::Horizontal, 5);
        let title = Label::new(None);
        title.set_markup(&format!("<b>{}</b>", clue_data.clue.clue_type.get_title()));
        title_box.append(&title);
        tooltip_box.append(&title_box);

        // Add description with example
        let desc_box = Box::new(Orientation::Horizontal, 5);

        // Create a temporary UI just for parsing templates
        match &clue_data.clue.clue_type {
            ClueType::Horizontal(horiz) => match horiz {
                HorizontalClueType::TwoAdjacent | HorizontalClueType::ThreeAdjacent => {
                    // Create template string with tiles and description
                    let mut template = String::new();
                    for (i, _) in clue_data.clue.assertions.iter().enumerate() {
                        if i > 0 {
                            template.push(' ');
                        }
                        template.push_str(&format!("{{t{}}}", i));
                    }
                    template.push_str(" are adjacent (forward, backward).");
                    desc_box.append(&self.parse_template(&template, clue_data));
                }
                HorizontalClueType::TwoApartNotMiddle => {
                    let template = "{t0} is two away from {t2}, without {t1} in the middle (forward, backward).";
                    desc_box.append(&self.parse_template(&template, clue_data));
                }
                HorizontalClueType::LeftOf => {
                    let template = "{t0} is left of {t1} (any number of tiles in between).";
                    desc_box.append(&self.parse_template(&template, clue_data));
                }
                HorizontalClueType::NotAdjacent => {
                    let template = "{t0} is not next to {t1} (forward, backward).";
                    desc_box.append(&self.parse_template(&template, clue_data));
                }
            },
            ClueType::Vertical(vert) => match vert {
                VerticalClueType::ThreeInColumn | VerticalClueType::TwoInColumn => {
                    let mut template = String::new();
                    for (i, _) in clue_data.clue.assertions.iter().enumerate() {
                        if i > 0 {
                            template.push(' ');
                        }
                        template.push_str(&format!("{{t{}}}", i));
                    }
                    template.push_str(" are in the same column.");
                    desc_box.append(&self.parse_template(&template, clue_data));
                }
                VerticalClueType::TwoInColumnWithout => {
                    let clue_assertions: Vec<(usize, &TileAssertion)> =
                        clue_data.clue.assertions.iter().enumerate().collect();

                    let positive_assertion_positions = clue_assertions
                        .iter()
                        .filter(|(_, ta)| ta.assertion)
                        .map(|(i, _)| format!("t{}", i))
                        .collect::<Vec<_>>();

                    let negative_assertion_positions = clue_assertions
                        .iter()
                        .filter(|(_, ta)| !ta.assertion)
                        .map(|(i, _)| format!("t{}", i))
                        .collect::<Vec<_>>();

                    assert!(positive_assertion_positions.len() == 2);
                    assert!(negative_assertion_positions.len() == 1);

                    let template = format!(
                        "{{{}}} and {{{}}} are in the same column, but {{{}}} isn't.",
                        positive_assertion_positions[0],
                        positive_assertion_positions[1],
                        negative_assertion_positions[0],
                    );
                    desc_box.append(&self.parse_template(&template, clue_data));
                }

                VerticalClueType::NotInSameColumn => {
                    let template = "{t0} is not in the same column as {t1}";
                    desc_box.append(&self.parse_template(&template, clue_data));
                }
                VerticalClueType::OneMatchesEither => {
                    let template =
                        "{t0} is either in the same column as {t1} or {t2}, but not both.";
                    desc_box.append(&self.parse_template(&template, clue_data));
                }
            },
        }

        tooltip_box.append(&desc_box);

        tooltip_box
    }

    pub fn new(
        resources: Rc<ResourceSet>,
        orientation: ClueOrientation,
        layout: CluesSizing,
    ) -> Self {
        let frame = Frame::builder()
            .name(&format!("clue-cell-frame-{}", orientation))
            .css_classes(["clue-cell-frame"])
            .build();

        let grid = Grid::new();
        let tooltip_data = Rc::new(RefCell::new(None));
        let tooltip_widget = Rc::new(RefCell::new(None));

        // Set up tooltip handling
        frame.set_has_tooltip(true);
        let tooltip_widget_clone = Rc::clone(&tooltip_widget);
        frame.connect_query_tooltip(move |_frame, _x, _y, _keyboard_mode, tooltip| {
            let widget = tooltip_widget_clone.borrow();
            if let Some(ref w) = *widget {
                tooltip.set_custom(Some(w));
            }
            true
        });

        grid.set_row_spacing(0);
        grid.set_column_spacing(0);

        // Create the three cells for this clue
        let mut cells = Vec::new();
        for i in 0..3 {
            let clue_cell = ClueTileUI::new(Rc::clone(&resources));
            match orientation {
                ClueOrientation::Horizontal => {
                    grid.attach(&clue_cell.frame, i as i32, 0, 1, 1);
                }
                ClueOrientation::Vertical => {
                    grid.attach(&clue_cell.frame, 0, i as i32, 1, 1);
                }
            }
            cells.push(clue_cell);
        }

        frame.set_child(Some(&grid));

        Self {
            frame,
            cells,
            orientation,
            tooltip_data,
            tooltip_widget,
            resources,
            layout,
        }
    }

    fn apply_layout(&self) {
        match self.orientation {
            ClueOrientation::Horizontal => {
                self.frame.set_size_request(
                    self.layout.horizontal_clue_panel.clue_dimensions.width,
                    self.layout.horizontal_clue_panel.clue_dimensions.height,
                );
            }
            ClueOrientation::Vertical => {
                self.frame.set_size_request(
                    self.layout.vertical_clue_panel.clue_dimensions.width,
                    self.layout.vertical_clue_panel.clue_dimensions.height,
                );

                if self.frame.has_css_class(NEW_GROUP_CSS_CLASS) {
                    self.frame
                        .set_margin_start(self.layout.vertical_clue_panel.group_spacing);
                } else {
                    self.frame.set_margin_start(0);
                }
            }
        }

        // Update individual tile sizes
        for cell in &self.cells {
            cell.update_layout(&self.layout);
        }
    }

    pub(crate) fn update_layout(&mut self, layout: &LayoutConfiguration) {
        self.layout = layout.clues.clone();
        self.apply_layout();
    }

    pub fn set_clue(&self, clue: Option<&Clue>, is_new_group: bool) {
        if let Some(clue) = clue {
            let tooltip_data = ClueTooltipData {
                clue: clue.clone(),
                resources: Rc::clone(&self.resources),
            };
            *self.tooltip_data.borrow_mut() = Some(tooltip_data);

            // Create new tooltip widget when clue changes
            let new_tooltip = self.create_tooltip_widget();
            *self.tooltip_widget.borrow_mut() = Some(new_tooltip);

            match self.orientation {
                ClueOrientation::Horizontal => self.set_horiz_clue(clue),
                ClueOrientation::Vertical => self.set_vert_clue(clue),
            }
            self.frame.set_visible(true);
            if clue.is_vertical() && is_new_group {
                self.frame.add_css_class(NEW_GROUP_CSS_CLASS);
            } else {
                self.frame.remove_css_class(NEW_GROUP_CSS_CLASS);
            }
            self.apply_layout();
        } else {
            *self.tooltip_data.borrow_mut() = None;
            *self.tooltip_widget.borrow_mut() = None;
            // clear
            for cell in &self.cells {
                cell.set_tile(None);
            }

            self.frame.set_visible(false);
            self.frame.remove_css_class(NEW_GROUP_CSS_CLASS);
        }
    }

    fn set_vert_clue(&self, clue: &Clue) {
        for tile_idx in 0..3 {
            self.cells[tile_idx].set_tile(clue.assertions.get(tile_idx));
        }

        match clue.clue_type {
            ClueType::Vertical(VerticalClueType::OneMatchesEither) => {
                self.cells[1].set_maybe();
                self.cells[2].set_maybe();
            }
            _ => {
                for tile_idx in 0..3 {
                    self.cells[tile_idx].set_tile(clue.assertions.get(tile_idx));
                }
            }
        }
    }

    fn set_horiz_clue(&self, clue: &Clue) {
        // Handle LeftOf clues specially
        match &clue.clue_type {
            ClueType::Horizontal(HorizontalClueType::LeftOf) => {
                self.cells[0].set_tile(clue.assertions.get(0));
                self.cells[1].set_tile(None);
                self.cells[2].set_tile(clue.assertions.get(1));

                self.cells[1].show_triple_dot();
            }
            _ => {
                for tile_idx in 0..3 {
                    self.cells[tile_idx].set_tile(clue.assertions.get(tile_idx));
                }
            }
        }
        //  else
    }

    pub fn highlight_for(&self, from_secs: std::time::Duration) {
        for cell in &self.cells {
            cell.highlight_for(from_secs);
        }
    }

    pub fn set_completed(&self, completed: bool) {
        if completed {
            self.frame.add_css_class("completed-clue");
        } else {
            self.frame.remove_css_class("completed-clue");
        }
        let opacity = if completed { 0.1 } else { 1.0 };
        self.frame.set_opacity(opacity);
    }
}

impl Drop for ClueUI {
    fn drop(&mut self) {
        // Clear tooltip data and widget first to drop any resource references
        *self.tooltip_data.borrow_mut() = None;
        *self.tooltip_widget.borrow_mut() = None;

        // Remove all cells from the grid
        if let Some(grid) = self.frame.child().and_then(|w| w.downcast::<Grid>().ok()) {
            for cell in &self.cells {
                if cell.frame.parent().is_some() {
                    grid.remove(&cell.frame);
                }
            }
            // Unparent the grid from frame
            self.frame.set_child(None::<&gtk::Widget>);
        }
    }
}
