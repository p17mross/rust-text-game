//! Functionality related to the [`Player`]'s state and actions 

use crate::combat::{Health, self};
use crate::items::Item;
use crate::config;
use crate::menu::{Menu, Screen, OptionList};
use crate::rooms::{Room, RoomGraph, init, RoomState};

/// The state of the player
#[derive(Debug)]
pub struct Player {
    /// Which [`Room`] the [`Player`] is in
    room: Room,
    /// The [`Player`]'s inventory
    pub inventory: Vec<Item>,
    /// The [`Player`]'s current health
    pub health: Health,
    /// The maximum health the [`Player`] can reach
    pub max_health: Health,

    /// The current state of the rooms
    room_graph: RoomGraph,
}

/// An action the [`Player`] can take outside of a battle 
#[derive(Debug)]
enum PassiveAction {
    /// Print the [`Player`]'s health
    CheckState,
    /// Go to a [`Room`] which is connected to the current one
    GoToRoom(Room),
    /// Use the [`Item`] at the given index into the [player's inventory][Player::inventory]
    UseItem(usize),
    /// Add the [`Item`] at the given index into the [current room's inventory][RoomState::items] to the [player's inventory][Player::inventory]
    PickUpItem(usize),
}

impl Player {
    /// Gets a shared reference to the current [`RoomState`]
    pub fn get_room_state(&self) -> &RoomState {
        self.room_graph.get_state(self.room)
    }

    /// Gets a mutable reference to the current [`RoomState`]
    pub fn get_room_state_mut(&mut self) -> &mut RoomState {
        self.room_graph.get_state_mut(self.room)
    }

    /// Prints a screen describing the current [`RoomState`]
    pub fn print_room(&self, menu: &mut impl Menu) {
        let screen = Screen {
            title: &format!("You are in the {}.", self.room.get_name()),
            content: self.room.get_description(),
        };
        
        menu.show_screen(screen);
    }

    /// Asks the user what [`PassiveAction`] to perform given the [`Player`]'s inventory and the current [`RoomState`]
    fn choose_passive_action(&self, menu: &mut impl Menu) -> PassiveAction {
        // Init lists of options and their string representations
        let mut options = vec![PassiveAction::CheckState];
        let mut options_str = vec!["Check how you're doing".to_string()];

        let room_state = self.room_graph.get_state(self.room);

        for connection in &room_state.connections {
            options.push(PassiveAction::GoToRoom(*connection));
            options_str.push(format!("Go to the {}", connection.get_name()));
        }

        for (i, item) in room_state.items.iter().enumerate() {
            options.push(PassiveAction::PickUpItem(i));
            options_str.push(format!("Pick up the {} - {}", item.get_name(), item.get_description()));
        }

        for (i, item) in self.inventory.iter().enumerate() {
            if let Item::Food(_) = item {
                options.push(PassiveAction::UseItem(i));
                options_str.push(format!("Eat your {}", item.get_name()));
            }
        }

        let option_list = OptionList::new(&options_str, "What do you do?");

        let choice = menu.show_option_list(option_list);

        options.swap_remove(choice)
    }

    /// Gets a [`PassiveAction`] from the user and carries it out
    pub fn take_passive_action(&mut self, menu: &mut impl Menu) {
        let action = self.choose_passive_action(menu);

        match action {
            PassiveAction::CheckState => self.print_state(menu),
            PassiveAction::GoToRoom(r) => self.room = r,
            PassiveAction::UseItem(i) => self.use_item(menu, i),
            PassiveAction::PickUpItem(i) => self.pick_up_item_from_room(i),
        }
    }

    /// Prints the [`Player`]'s health
    fn print_state(&self, menu: &mut impl Menu) {
        let screen = Screen {
            title: "You take a moment to rest and check your body for injuries",
            content: &format!("You are at {}/{} HP", self.health, self.max_health),
        };

        menu.show_screen(screen);
    }

    /// Uses the [`Item`] at the given index into the [`Player`]'s inventory
    fn use_item(&mut self, menu: &mut impl Menu, i: usize) {
        match &self.inventory[i] {
            Item::Food(f) => {
                let prev_health = self.health;
                self.health.heal_to_max(f.heals_for, self.max_health);

                let screen = Screen {
                    title: &format!("You ate your {}", f.name),
                    content: &format!("You are healed by {} HP.\nYou are now at {}/{} HP.", self.health - prev_health, self.health, self.max_health),
                };

                menu.show_screen(screen);

                self.inventory.remove(i);
            },
            Item::Weapon(_) => {
                panic!("Weapons cannot be used outside of combat")
            }
        }
    }

    /// Removes an [`Item`] from the current [`RoomState`] at the specified index and adds it to the [player's inventory][Player::inventory]
    fn pick_up_item_from_room(&mut self, i: usize) {
        let room_state = self.room_graph.get_state_mut(self.room);
        let item = room_state.items.remove(i);
        self.pick_up_item(item);
    }

    /// Add an item to the [player's inventory][Player::inventory]
    pub fn pick_up_item(&mut self, item: Item) {
        // TODO: max inventory size
        self.inventory.push(item);
    }

    /// Get the user to choose a [combat action][combat::Action] to perform
    pub fn choose_combat_action(&self, menu: &mut impl Menu) -> combat::Action {
        // Init lists of options and their string representations
        let mut options = vec![
            combat::Action::Nothing, 
            combat::Action::DodgeLeft, 
            combat::Action::DodgeRight
        ];
        let mut options_str = vec![
            "Do nothing".to_string(),
            "Dodge to the left".to_string(),
            "Dodge to the right".to_string(),
        ];

        // Add actions for items
        for (i, item) in self.inventory.iter().enumerate() {
            match item {
                Item::Food(f) => {
                    options.push(combat::Action::EatFood(i));
                    options_str.push(format!("Eat your {}", f.name));
                }
                Item::Weapon(w) => {
                    options.push(combat::Action::AttackStraight(i));
                    options_str.push(format!("Attack with your {}", w.name));
                }
            }
        }

        // Get the user to pick an option
        let list = OptionList::new(&options_str, "What do you do?");
        let choice = menu.show_option_list(list);

        // If the action was an attack, get the user to pick which direction to aim it
        if let combat::Action::AttackStraight(i) = options[choice] {
            let options = &[
                "Attack Left".to_string(),
                "Attack Straight".to_string(),
                "Attack Right".to_string(),
            ];
            let list = OptionList::new(options, "Which way do you attack?");

            let direction = menu.show_option_list(list);
            
            match direction {
                0 => combat::Action::AttackLeft(i),
                1 => combat::Action::AttackStraight(i),
                2 => combat::Action::AttackRight(i),
                _ => unreachable!()
            }
        } else {
            options.swap_remove(choice)
        }

    }

    /// Get a [`String`] describing the [`Player`] performing a [combat action][combat::Action]
    pub fn describe_combat_action(&self, action: combat::Action) -> String {
        use combat::Action::*;

        match action {
            AttackLeft(w) => format!("You attack to the left with your {}", self.inventory[w].get_name()),
            AttackRight(w) => format!("You attack to the right with your {}", self.inventory[w].get_name()),
            AttackStraight(w) => format!("You attack in front of you with your {}", self.inventory[w].get_name()),
            EatFood(f) => format!("You attempt to eat your {}", self.inventory[f].get_name()),
            
            DodgeLeft => "You dodge to the left".to_string(),
            DodgeRight => "You dodge to the right".to_string(),
            Nothing => "You do nothing".to_string(),
        }
    }
}

impl Player {
    /// Initialise a new [`Player`]
    pub fn init() -> Self {
        Self {
            room: Room::Bridge,
            inventory: Vec::new(),
            health: config::PLAYER_START_HEALTH,
            max_health: config::PLAYER_START_MAX_HEALTH,

            room_graph: init(),
        }
    }
}