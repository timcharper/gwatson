use crate::game::game_event::GameEvent;
use crate::game::game_state::GameState;
use crate::game::settings::Settings;
use crate::game::stats_manager::StatsManager;
use crate::model::Difficulty;
use crate::ui::stats_dialog::StatsDialog;
use crate::ui::timer_button_ui::TimerButtonUI;
use glib::timeout_add_local_once;
use gtk::gdk::Display;
use gtk::prelude::*;
use gtk::{Application, ApplicationWindow, Button, Label, Orientation};
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use super::ResourceSet;

fn hint_button_handler(
    window_ref: &Rc<ApplicationWindow>,
    game_state: &Rc<RefCell<GameState>>,
    resources: &Rc<ResourceSet>,
) -> impl Fn(&Button) {
    let window_ref = Rc::clone(&window_ref);
    let game_state = Rc::clone(&game_state);
    let resources_hint = Rc::clone(&resources);

    move |button| {
        let board_is_incorrect = game_state.borrow().current_board.is_incorrect();
        log::trace!(target: "window", "Handling hint button click");
        if board_is_incorrect {
            log::trace!(target: "window", "Board is incorrect, showing rewind dialog");
            GameEvent::dispatch_event(&window_ref, GameEvent::IncrementHintsUsed);
            // Play game over sound using a MediaStream
            let media = resources_hint.random_lose_sound();
            media.play();

            // show dialog
            let dialog = gtk::MessageDialog::new(
                button
                    .root()
                    .and_then(|r| r.downcast::<gtk::Window>().ok())
                    .as_ref(),
                gtk::DialogFlags::MODAL,
                gtk::MessageType::Info,
                gtk::ButtonsType::OkCancel,
                "Sorry, that's not quite right. Click OK to rewind to the last correct state.",
            );
            let window_ref = Rc::clone(&window_ref);
            dialog.connect_response(move |dialog, response| {
                log::trace!(target: "window", "Dialog response: {:?}", response);
                if response == gtk::ResponseType::Ok {
                    GameEvent::dispatch_event(&window_ref, GameEvent::RewindLastGood);
                }
                dialog.close();
            });
            dialog.show();
        } else {
            log::trace!(target: "window", "Board is correct, showing hint");
            GameEvent::dispatch_event(&window_ref, GameEvent::ShowHint);
            button.set_sensitive(false);
            let button = button.clone();
            timeout_add_local_once(Duration::from_secs(1), move || {
                log::trace!(target: "window", "Re-enabling hint button");
                button.set_sensitive(true);
            });
        }
    }
}

fn submit_handler(
    window_ref: &Rc<ApplicationWindow>,
    state_submit: &Rc<RefCell<GameState>>,
    manager_submit: &Rc<RefCell<StatsManager>>,
    resources: &Rc<ResourceSet>,
) -> impl Fn(&Button) {
    let state_submit = Rc::clone(&state_submit);
    let manager_submit = Rc::clone(&manager_submit);
    let resources = Rc::clone(&resources);
    let window_ref = Rc::clone(&window_ref);

    move |button| {
        let state = state_submit.try_borrow().ok().and_then(|gs| {
            manager_submit
                .try_borrow_mut()
                .ok()
                .and_then(|sm| Some((gs, sm)))
        });
        if let Some((state, mut stats_manager)) = state {
            if state.current_board.is_complete() && !state.current_board.is_incorrect() {
                button.remove_css_class("submit-ready"); // Stop blinking once clicked
                let media = resources.random_win_sound();
                media.play();

                // Record completion and show stats
                let stats = state.get_game_stats();
                let grid_size = state.current_board.solution.n_rows;

                if let Err(e) = stats_manager.record_game(&stats) {
                    log::error!(target: "window", "Failed to record game stats: {}", e);
                }

                if let Some(window) = button
                    .root()
                    .and_then(|r| r.downcast::<ApplicationWindow>().ok())
                {
                    // Drop the mutable borrow before showing stats
                    let window_ref = Rc::clone(&window_ref);
                    StatsDialog::show(&window, &state, &stats_manager, Some(stats), move || {
                        GameEvent::dispatch_event(&window_ref, GameEvent::NewGame(grid_size));
                    });
                }
            } else {
                let dialog = gtk::MessageDialog::new(
                    button
                        .root()
                        .and_then(|r| r.downcast::<gtk::Window>().ok())
                        .as_ref(),
                    gtk::DialogFlags::MODAL,
                    gtk::MessageType::Info,
                    gtk::ButtonsType::OkCancel,
                    "Sorry, that's not quite right. Click OK to rewind to the last correct state.",
                );

                // Play game over sound using a MediaStream
                let media = resources.random_lose_sound();
                media.play();

                let window_ref = Rc::clone(&window_ref);
                dialog.connect_response(move |dialog, response| {
                    if response == gtk::ResponseType::Ok {
                        GameEvent::dispatch_event(&window_ref, GameEvent::RewindLastGood);
                    }
                    dialog.close();
                });
                dialog.show();
            }
        }
    }
}

pub fn build_ui(app: &Application) {
    let settings = Rc::new(RefCell::new(Settings::load()));
    let resources = Rc::new(ResourceSet::new());
    let window = Rc::new(
        ApplicationWindow::builder()
            .application(app)
            .title("GWatson Logic Puzzle")
            .resizable(true)
            .build(),
    );

    // Set up keyboard shortcuts
    app.set_accels_for_action("win.undo", &["<Control>z"]);
    app.set_accels_for_action("win.redo", &["<Control><Shift>z"]);
    app.set_accels_for_action("win.new-game", &["<Control>n"]);
    app.set_accels_for_action("win.pause", &["space"]);

    // Create menu model for hamburger menu
    let menu = gtk::gio::Menu::new();

    // Simplified menu with single New Game option
    menu.append(Some("New Game"), Some("win.new-game"));
    menu.append(Some("Statistics"), Some("win.statistics"));
    menu.append(Some("About"), Some("win.about"));

    // Add menu button to header bar
    let header_bar = gtk::HeaderBar::new();

    // Create difficulty selector dropdown with label
    let difficulty_box = gtk::Box::builder()
        .orientation(Orientation::Horizontal)
        .spacing(5)
        .build();

    let difficulty_label = gtk::Label::new(Some("Difficulty:"));
    difficulty_box.append(&difficulty_label);

    let difficulty_selector = gtk::DropDown::from_strings(&[
        &Difficulty::Easy.to_string(),
        &Difficulty::Moderate.to_string(),
        &Difficulty::Hard.to_string(),
        &Difficulty::Veteran.to_string(),
    ]);
    difficulty_selector.set_tooltip_text(Some("Select Difficulty"));
    difficulty_box.append(&difficulty_selector);

    // Set initial selection based on current settings
    let current_difficulty = settings.borrow().difficulty;
    difficulty_selector.set_selected(match current_difficulty {
        Difficulty::Easy => 0,
        Difficulty::Moderate => 1,
        Difficulty::Hard => 2,
        Difficulty::Veteran => 3,
    });

    // Handle difficulty changes
    let settings_ref = Rc::clone(&settings);
    let window_ref = Rc::clone(&window);
    difficulty_selector.connect_selected_notify(move |selector| {
        let new_difficulty = match selector.selected() {
            0 => Difficulty::Easy,
            1 => Difficulty::Moderate,
            2 => Difficulty::Hard,
            3 => Difficulty::Veteran,
            _ => return,
        };
        settings_ref.borrow_mut().difficulty = new_difficulty;
        let _ = settings_ref.borrow().save();
        let grid_size = new_difficulty.grid_size();
        GameEvent::dispatch_event(&window_ref, GameEvent::NewGame(grid_size));

        // Set window to minimum size after a short delay to ensure new game is rendered
        let window_ref = Rc::clone(&window_ref);
        glib::timeout_add_local_once(std::time::Duration::from_millis(100), move || {
            window_ref.set_default_size(1, 1); // This triggers the window to shrink to its minimum size
            window_ref.queue_resize();
        });
    });

    header_bar.pack_start(&difficulty_box);

    // Create buttons first
    let undo_button = Rc::new(Button::from_icon_name("edit-undo-symbolic"));
    let redo_button = Rc::new(Button::from_icon_name("edit-redo-symbolic"));
    let solve_button = Button::with_label("Solve");
    let hint_button = Button::from_icon_name("view-reveal-symbolic");
    let submit_button = Rc::new(Button::with_label("Submit"));
    submit_button.set_sensitive(false); // Initially disabled

    // Add tooltips
    undo_button.set_tooltip_text(Some("Undo (Ctrl+Z)"));
    redo_button.set_tooltip_text(Some("Redo (Ctrl+Shift+Z)"));
    hint_button.set_tooltip_text(Some("Show Hint"));

    // Create game state first to know how many clue cells we need
    let game_state = Rc::new(RefCell::new(GameState::new(
        &submit_button,
        &undo_button,
        &redo_button,
        &window,
        &resources,
    )));

    // Create left side box for timer and hints
    let left_box = gtk::Box::builder()
        .orientation(Orientation::Horizontal)
        .spacing(10) // Slightly larger spacing between groups
        .build();

    // Create pause button
    let timer_button = TimerButtonUI::new(&window);
    left_box.append(timer_button.button.as_ref());
    left_box.append(game_state.borrow().game_info.timer_label.as_ref());
    let hints_label = Label::new(Some("Hints: "));
    hints_label.set_css_classes(&["hints-label"]);
    left_box.append(&hints_label);
    left_box.append(game_state.borrow().game_info.hints_label.as_ref());

    header_bar.pack_start(&left_box);

    // Create right side box for controls
    let right_box = gtk::Box::builder()
        .orientation(Orientation::Horizontal)
        .spacing(5)
        .css_classes(["menu-box"])
        .build();

    right_box.append(undo_button.as_ref());
    right_box.append(redo_button.as_ref());
    if GameState::is_debug_mode() {
        right_box.append(&solve_button);
    }
    right_box.append(&hint_button);
    right_box.append(submit_button.as_ref());

    let menu_button = gtk::MenuButton::builder()
        .icon_name("open-menu-symbolic")
        .menu_model(&menu)
        .build();

    // Pack the controls on the right
    header_bar.pack_end(&menu_button); // Hamburger menu goes last
    header_bar.pack_end(&right_box); // Controls go before hamburger menu

    window.set_titlebar(Some(&header_bar));

    // Create main container
    let main_box = gtk::Box::builder()
        .orientation(Orientation::Vertical)
        .spacing(10)
        .build();

    // Create game area with puzzle and horizontal clues side by side
    let game_box = gtk::Box::builder()
        .orientation(Orientation::Horizontal)
        .spacing(10)
        .build();

    // Create a vertical box for puzzle grid and vertical clues
    let puzzle_vertical_box = gtk::Box::builder()
        .orientation(Orientation::Vertical)
        .spacing(10)
        .build();

    // Remove the old button_box since controls are now in header
    let stats_manager = Rc::new(RefCell::new(StatsManager::new()));

    let window_ref = Rc::clone(&window);
    solve_button.connect_clicked(move |_| {
        GameEvent::dispatch_event(&window_ref, GameEvent::Solve);
    });

    // Connect hint button
    let window_ref = Rc::clone(&window);
    hint_button.connect_clicked(hint_button_handler(&window_ref, &game_state, &resources));

    // Wire up submit button handler
    submit_button.connect_clicked(submit_handler(
        &window,
        &game_state,
        &stats_manager,
        &resources,
    ));

    // Set up game event loop
    let action = gtk::gio::SimpleAction::new(
        "game-event",
        Some(&gtk::glib::VariantType::new("s").unwrap()),
    );
    let game_state_ref = Rc::clone(&game_state);
    action.connect_activate(move |_, variant| {
        if let Some(variant) = variant {
            if let Some(event) = GameEvent::from_variant(variant) {
                if let Ok(mut state) = game_state_ref.try_borrow_mut() {
                    state.handle_event(event);
                } else {
                    log::error!("Failed to borrow game state");
                }
            }
        }
    });
    window.add_action(&action);

    // Initialize game with saved difficulty
    let initial_size = settings.borrow().difficulty.grid_size();
    game_state
        .borrow_mut()
        .handle_event(GameEvent::NewGame(initial_size));

    // Add CSS for selected cells
    let provider = gtk::CssProvider::new();
    provider.load_from_resource("/org/gwatson/style.css");

    gtk::style_context_add_provider_for_display(
        Display::default()
            .as_ref()
            .expect("Could not connect to a display."),
        &provider,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );

    // Assemble the UI
    puzzle_vertical_box.append(&game_state.borrow().puzzle_grid_ui.grid);
    puzzle_vertical_box.append(&game_state.borrow().clue_set_ui.vertical_grid);
    puzzle_vertical_box.set_hexpand(false);

    game_box.append(&puzzle_vertical_box);
    game_box.append(&game_state.borrow().clue_set_ui.horizontal_grid);
    game_box.append(&game_state.borrow().puzzle_grid_ui.pause_label);

    main_box.append(&game_box);

    window.set_child(Some(&main_box));
    window.present();

    // Add actions for keyboard shortcuts and menu items
    let action_undo = gtk::gio::SimpleAction::new("undo", None);
    let window_ref = Rc::clone(&window);
    action_undo.connect_activate(move |_, _| {
        GameEvent::dispatch_event(&window_ref, GameEvent::Undo);
    });
    window.add_action(&action_undo);

    let action_redo = gtk::gio::SimpleAction::new("redo", None);
    let window_ref = Rc::clone(&window);
    action_redo.connect_activate(move |_, _| {
        GameEvent::dispatch_event(&window_ref, GameEvent::Redo);
    });
    window.add_action(&action_redo);

    // Connect undo/redo buttons to the actions
    undo_button.set_action_name(Some("win.undo"));
    redo_button.set_action_name(Some("win.redo"));

    // Add new game action that uses current difficulty
    let action_new_game = gtk::gio::SimpleAction::new("new-game", None);
    let window_ref = Rc::clone(&window);
    let settings_ref: Rc<RefCell<Settings>> = Rc::clone(&settings);
    action_new_game.connect_activate(move |_, _| {
        let difficulty = settings_ref.borrow().difficulty;
        let grid_size = difficulty.grid_size();
        GameEvent::dispatch_event(&window_ref, GameEvent::NewGame(grid_size));
    });
    window.add_action(&action_new_game);

    let action_statistics = gtk::gio::SimpleAction::new("statistics", None);
    let game_state_stats = Rc::clone(&game_state);
    let stats_manager_stats = Rc::clone(&stats_manager);
    action_statistics.connect_activate(move |_, _| {
        if let Some(window) = game_state_stats.try_borrow().ok().and_then(|state| {
            state
                .submit_button
                .as_ref()
                .root()
                .and_then(|r| r.downcast::<ApplicationWindow>().ok())
        }) {
            StatsDialog::show(
                &window,
                &game_state_stats.borrow(),
                &stats_manager_stats.borrow_mut(),
                None,
                || {},
            );
        }
    });
    window.add_action(&action_statistics);

    let action_about = gtk::gio::SimpleAction::new("about", None);
    action_about.connect_activate(move |_, _| {
        let dialog = gtk::AboutDialog::builder()
            .program_name("GWatson Logic Puzzle")
            .version("1.0")
            .authors(vec!["Tim Harper"])
            .website("https://github.com/timcharper/gwatson")
            .website_label("GitHub Repository")
            .license_type(gtk::License::MitX11)
            .build();
        dialog.present();
    });
    window.add_action(&action_about);
}