use std::cell::RefCell;

const CARD_W: f64 = 104.0;
const CARD_H: f64 = 150.0;
const BOARD_W: f64 = 1100.0;
const GAP: f64 = 18.0;
const TOP: f64 = 30.0;
const LEFT: f64 = 34.0;
const TABLEAU_TOP: f64 = 214.0;
const FACE_DOWN_STEP: f64 = 25.0;
const FACE_UP_STEP: f64 = 38.0;
const SCORE_WASTE_TO_TABLEAU: i32 = 5;
const SCORE_TO_FOUNDATION: i32 = 10;
const SCORE_TURN_TABLEAU: i32 = 5;
const SCORE_FOUNDATION_TO_TABLEAU: i32 = -15;
const SCORE_RECYCLE_STOCK: i32 = -100;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Card {
    suit: u8,
    rank: u8,
    face_up: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Location {
    Waste,
    Foundation(usize),
    Tableau(usize, usize),
}

#[derive(Clone, Copy, Debug)]
struct Selection {
    loc: Location,
}

#[derive(Clone, Copy, Debug)]
struct RenderCard {
    x: f64,
    y: f64,
    rank: u8,
    suit: u8,
    face_up: bool,
    selected: bool,
}

#[derive(Clone, Debug)]
struct Snapshot {
    stock: Vec<Card>,
    waste: Vec<Card>,
    foundations: [Vec<Card>; 4],
    tableau: [Vec<Card>; 7],
    selected: Option<Selection>,
    moves: u32,
    score: i32,
}

#[derive(Debug)]
struct Game {
    stock: Vec<Card>,
    waste: Vec<Card>,
    foundations: [Vec<Card>; 4],
    tableau: [Vec<Card>; 7],
    selected: Option<Selection>,
    moves: u32,
    score: i32,
    undo_stack: Vec<Snapshot>,
}

impl Game {
    fn new(seed: u32) -> Self {
        let mut deck = shuffled_deck(seed);
        let mut tableau: [Vec<Card>; 7] = std::array::from_fn(|_| Vec::new());

        for pile_idx in 0..7 {
            for card_idx in 0..=pile_idx {
                let mut card = deck.pop().expect("deck has enough cards");
                card.face_up = card_idx == pile_idx;
                tableau[pile_idx].push(card);
            }
        }

        Self {
            stock: deck,
            waste: Vec::new(),
            foundations: std::array::from_fn(|_| Vec::new()),
            tableau,
            selected: None,
            moves: 0,
            score: 0,
            undo_stack: Vec::new(),
        }
    }

    fn click(&mut self, x: f64, y: f64) {
        if self.won() {
            self.selected = None;
            return;
        }

        if point_in_rect(x, y, stock_x(), TOP, CARD_W, CARD_H) {
            self.selected = None;
            self.draw_stock();
            return;
        }

        if let Some(target) = self.hit_location(x, y) {
            if let Some(selection) = self.selected {
                if selection.loc == target {
                    self.selected = None;
                    return;
                }

                if self.try_move(selection.loc, target) {
                    self.selected = None;
                    self.flip_open_tableau_cards();
                    return;
                }
            }

            self.selected = self
                .selectable_location(target)
                .map(|loc| Selection { loc });
            return;
        }

        if let Some(selection) = self.selected {
            if self.try_move_to_empty_foundation(selection.loc, x, y)
                || self.try_move_to_empty_tableau(selection.loc, x, y)
            {
                self.selected = None;
                self.flip_open_tableau_cards();
                return;
            }
        }

        self.selected = None;
    }

    fn auto_move_card(&mut self, x: f64, y: f64) -> bool {
        if self.won() {
            self.selected = None;
            return false;
        }

        let Some(source) = self.hit_location(x, y) else {
            self.selected = None;
            return false;
        };

        let Some(card) = self.source_first_card(source) else {
            self.selected = None;
            return false;
        };

        if !card.face_up {
            self.selected = None;
            return false;
        }

        if let Some(top_card) = self.source_top_card(source) {
            for idx in 0..4 {
                if can_place_on_foundation(top_card, &self.foundations[idx]) {
                    let moved = self.try_move_to_foundation(source, idx);
                    if moved {
                        self.selected = None;
                        self.flip_open_tableau_cards();
                    }
                    return moved;
                }
            }
        }

        for idx in 0..7 {
            if self.try_move_to_tableau(source, idx) {
                self.selected = None;
                self.flip_open_tableau_cards();
                return true;
            }
        }

        self.selected = None;
        false
    }

    fn auto_play_step(&mut self) -> bool {
        if self.won() {
            self.selected = None;
            return false;
        }

        self.selected = None;

        if self.auto_play_foundation_move()
            || self.auto_play_waste_to_tableau()
            || self.auto_play_revealing_tableau_move()
        {
            self.flip_open_tableau_cards();
            return true;
        }

        if !self.stock.is_empty() {
            self.draw_stock();
            return true;
        }

        false
    }

    fn auto_play_foundation_move(&mut self) -> bool {
        if self.auto_play_source_to_foundation(Location::Waste) {
            return true;
        }

        for pile_idx in 0..7 {
            let Some(card_idx) = self.tableau[pile_idx].len().checked_sub(1) else {
                continue;
            };

            if self.auto_play_source_to_foundation(Location::Tableau(pile_idx, card_idx)) {
                return true;
            }
        }

        false
    }

    fn auto_play_source_to_foundation(&mut self, source: Location) -> bool {
        let Some(card) = self.source_top_card(source) else {
            return false;
        };

        if !card.face_up {
            return false;
        }

        for idx in 0..4 {
            if self.try_move_to_foundation(source, idx) {
                return true;
            }
        }

        false
    }

    fn auto_play_waste_to_tableau(&mut self) -> bool {
        if self.waste.last().is_none_or(|card| !card.face_up) {
            return false;
        }

        for pile_idx in 0..7 {
            if self.try_move_to_tableau(Location::Waste, pile_idx) {
                return true;
            }
        }

        false
    }

    fn auto_play_revealing_tableau_move(&mut self) -> bool {
        for source_pile_idx in 0..7 {
            let pile_len = self.tableau[source_pile_idx].len();
            if pile_len == 0 {
                continue;
            }

            for card_idx in 0..pile_len {
                let card = self.tableau[source_pile_idx][card_idx];
                if !card.face_up || !self.move_reveals_tableau_card(source_pile_idx, card_idx) {
                    continue;
                }

                let source = Location::Tableau(source_pile_idx, card_idx);
                for target_pile_idx in 0..7 {
                    if self.try_move_to_tableau(source, target_pile_idx) {
                        return true;
                    }
                }
            }
        }

        false
    }

    fn move_reveals_tableau_card(&self, pile_idx: usize, card_idx: usize) -> bool {
        card_idx > 0 && !self.tableau[pile_idx][card_idx - 1].face_up
    }

    fn draw_stock(&mut self) {
        if !self.stock.is_empty() {
            self.save_undo();
            let mut card = self.stock.pop().expect("stock is not empty");
            card.face_up = true;
            self.waste.push(card);
            self.moves += 1;
            return;
        }

        if self.waste.is_empty() {
            return;
        }

        self.save_undo();
        while let Some(mut card) = self.waste.pop() {
            card.face_up = false;
            self.stock.push(card);
        }
        self.moves += 1;
        self.score += SCORE_RECYCLE_STOCK;
    }

    fn hit_location(&self, x: f64, y: f64) -> Option<Location> {
        for pile_idx in (0..7).rev() {
            let pile = &self.tableau[pile_idx];
            for card_idx in (0..pile.len()).rev() {
                let (card_x, card_y) = tableau_card_pos(pile, pile_idx, card_idx);
                let hit_h = if card_idx + 1 == pile.len() {
                    CARD_H
                } else if pile[card_idx].face_up {
                    FACE_UP_STEP
                } else {
                    FACE_DOWN_STEP
                };

                if point_in_rect(x, y, card_x, card_y, CARD_W, hit_h) {
                    return Some(Location::Tableau(pile_idx, card_idx));
                }
            }
        }

        if point_in_rect(x, y, waste_x(), TOP, CARD_W, CARD_H) && !self.waste.is_empty() {
            return Some(Location::Waste);
        }

        for idx in 0..4 {
            if point_in_rect(x, y, foundation_x(idx), TOP, CARD_W, CARD_H)
                && !self.foundations[idx].is_empty()
            {
                return Some(Location::Foundation(idx));
            }
        }

        None
    }

    fn selectable_location(&mut self, loc: Location) -> Option<Location> {
        match loc {
            Location::Waste => self.waste.last().map(|_| loc),
            Location::Foundation(idx) => self.foundations[idx].last().map(|_| loc),
            Location::Tableau(pile_idx, card_idx) => {
                let pile = &self.tableau[pile_idx];
                let is_top_card = card_idx + 1 == pile.len();
                let card = pile.get(card_idx)?;

                if !card.face_up {
                    if is_top_card {
                        self.save_undo();
                        self.tableau[pile_idx][card_idx].face_up = true;
                        self.moves += 1;
                        self.score += SCORE_TURN_TABLEAU;
                    }
                    return None;
                }

                Some(loc)
            }
        }
    }

    fn try_move(&mut self, source: Location, target: Location) -> bool {
        match target {
            Location::Foundation(idx) => self.try_move_to_foundation(source, idx),
            Location::Tableau(idx, _) => self.try_move_to_tableau(source, idx),
            Location::Waste => false,
        }
    }

    fn try_move_to_empty_foundation(&mut self, source: Location, x: f64, y: f64) -> bool {
        for idx in 0..4 {
            if self.foundations[idx].is_empty()
                && point_in_rect(x, y, foundation_x(idx), TOP, CARD_W, CARD_H)
            {
                return self.try_move_to_foundation(source, idx);
            }
        }
        false
    }

    fn try_move_to_empty_tableau(&mut self, source: Location, x: f64, y: f64) -> bool {
        for idx in 0..7 {
            if self.tableau[idx].is_empty()
                && point_in_rect(x, y, tableau_x(idx), TABLEAU_TOP, CARD_W, CARD_H)
            {
                return self.try_move_to_tableau(source, idx);
            }
        }
        false
    }

    fn try_move_to_foundation(&mut self, source: Location, foundation_idx: usize) -> bool {
        let Some(card) = self.source_top_card(source) else {
            return false;
        };

        if !can_place_on_foundation(card, &self.foundations[foundation_idx]) {
            return false;
        }

        self.save_undo();
        let moved = self.take_cards(source);
        self.foundations[foundation_idx].extend(moved);
        self.moves += 1;
        self.score += SCORE_TO_FOUNDATION;
        true
    }

    fn try_move_to_tableau(&mut self, source: Location, pile_idx: usize) -> bool {
        let moving = self.source_first_card(source);
        let Some(card) = moving else {
            return false;
        };

        if matches!(source, Location::Tableau(idx, _) if idx == pile_idx) {
            return false;
        }

        if !can_place_on_tableau(card, &self.tableau[pile_idx]) {
            return false;
        }

        self.save_undo();
        let moved = self.take_cards(source);
        self.tableau[pile_idx].extend(moved);
        self.moves += 1;
        self.score += match source {
            Location::Waste => SCORE_WASTE_TO_TABLEAU,
            Location::Foundation(_) => SCORE_FOUNDATION_TO_TABLEAU,
            Location::Tableau(_, _) => 0,
        };
        true
    }

    fn source_top_card(&self, loc: Location) -> Option<Card> {
        match loc {
            Location::Waste => self.waste.last().copied(),
            Location::Foundation(idx) => self.foundations[idx].last().copied(),
            Location::Tableau(pile_idx, card_idx) => {
                let pile = &self.tableau[pile_idx];
                if card_idx + 1 == pile.len() {
                    pile.get(card_idx).copied()
                } else {
                    None
                }
            }
        }
    }

    fn source_first_card(&self, loc: Location) -> Option<Card> {
        match loc {
            Location::Waste => self.waste.last().copied(),
            Location::Foundation(idx) => self.foundations[idx].last().copied(),
            Location::Tableau(pile_idx, card_idx) => self.tableau[pile_idx].get(card_idx).copied(),
        }
    }

    fn take_cards(&mut self, loc: Location) -> Vec<Card> {
        match loc {
            Location::Waste => self.waste.pop().into_iter().collect(),
            Location::Foundation(idx) => self.foundations[idx].pop().into_iter().collect(),
            Location::Tableau(pile_idx, card_idx) => self.tableau[pile_idx].split_off(card_idx),
        }
    }

    fn flip_open_tableau_cards(&mut self) {
        for pile in &mut self.tableau {
            if let Some(card) = pile.last_mut() {
                if !card.face_up {
                    card.face_up = true;
                    self.score += SCORE_TURN_TABLEAU;
                }
            }
        }
    }

    fn render_cards(&self) -> Vec<RenderCard> {
        let mut cards = Vec::with_capacity(52);

        if self.waste.len() >= 2 {
            let card = self.waste[self.waste.len() - 2];
            cards.push(RenderCard {
                x: waste_x(),
                y: TOP,
                rank: card.rank,
                suit: card.suit,
                face_up: true,
                selected: false,
            });
        }

        if let Some(card) = self.waste.last() {
            cards.push(RenderCard {
                x: waste_x(),
                y: TOP,
                rank: card.rank,
                suit: card.suit,
                face_up: true,
                selected: self.is_selected(Location::Waste),
            });
        }

        for idx in 0..4 {
            if let Some(card) = self.foundations[idx].last() {
                cards.push(RenderCard {
                    x: foundation_x(idx),
                    y: TOP,
                    rank: card.rank,
                    suit: card.suit,
                    face_up: true,
                    selected: self.is_selected(Location::Foundation(idx)),
                });
            }
        }

        for pile_idx in 0..7 {
            let pile = &self.tableau[pile_idx];
            for card_idx in 0..pile.len() {
                let card = pile[card_idx];
                let (x, y) = tableau_card_pos(pile, pile_idx, card_idx);
                cards.push(RenderCard {
                    x,
                    y,
                    rank: card.rank,
                    suit: card.suit,
                    face_up: card.face_up,
                    selected: self.is_selected(Location::Tableau(pile_idx, card_idx)),
                });
            }
        }

        cards
    }

    fn is_selected(&self, loc: Location) -> bool {
        let Some(selection) = self.selected else {
            return false;
        };

        match (selection.loc, loc) {
            (Location::Waste, Location::Waste) => true,
            (Location::Foundation(a), Location::Foundation(b)) => a == b,
            (Location::Tableau(a_pile, a_idx), Location::Tableau(b_pile, b_idx)) => {
                a_pile == b_pile && b_idx >= a_idx
            }
            _ => false,
        }
    }

    fn won(&self) -> bool {
        self.foundations.iter().all(|pile| pile.len() == 13)
    }

    fn save_undo(&mut self) {
        self.undo_stack.push(Snapshot {
            stock: self.stock.clone(),
            waste: self.waste.clone(),
            foundations: self.foundations.clone(),
            tableau: self.tableau.clone(),
            selected: None,
            moves: self.moves,
            score: self.score,
        });
    }

    fn undo(&mut self) -> bool {
        let Some(snapshot) = self.undo_stack.pop() else {
            return false;
        };

        self.stock = snapshot.stock;
        self.waste = snapshot.waste;
        self.foundations = snapshot.foundations;
        self.tableau = snapshot.tableau;
        self.selected = snapshot.selected;
        self.moves = snapshot.moves;
        self.score = snapshot.score;
        true
    }

    fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }
}

fn shuffled_deck(seed: u32) -> Vec<Card> {
    let mut deck = Vec::with_capacity(52);
    for suit in 0..4 {
        for rank in 1..=13 {
            deck.push(Card {
                suit,
                rank,
                face_up: false,
            });
        }
    }

    let mut rng = XorShift32::new(seed);
    for idx in (1..deck.len()).rev() {
        let swap_idx = (rng.next() as usize) % (idx + 1);
        deck.swap(idx, swap_idx);
    }
    deck
}

#[derive(Debug)]
struct XorShift32 {
    state: u32,
}

impl XorShift32 {
    fn new(seed: u32) -> Self {
        Self {
            state: if seed == 0 { 0x9e37_79b9 } else { seed },
        }
    }

    fn next(&mut self) -> u32 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.state = x;
        x
    }
}

fn can_place_on_foundation(card: Card, foundation: &[Card]) -> bool {
    match foundation.last() {
        None => card.rank == 1,
        Some(top) => top.suit == card.suit && top.rank + 1 == card.rank,
    }
}

fn can_place_on_tableau(card: Card, pile: &[Card]) -> bool {
    match pile.last() {
        None => card.rank == 13,
        Some(top) => {
            top.face_up && is_red(top.suit) != is_red(card.suit) && top.rank == card.rank + 1
        }
    }
}

fn is_red(suit: u8) -> bool {
    suit == 1 || suit == 2
}

fn point_in_rect(px: f64, py: f64, x: f64, y: f64, w: f64, h: f64) -> bool {
    px >= x && px <= x + w && py >= y && py <= y + h
}

fn stock_x() -> f64 {
    LEFT
}

fn waste_x() -> f64 {
    LEFT + CARD_W + GAP
}

fn foundation_x(idx: usize) -> f64 {
    BOARD_W - LEFT - (4.0 - idx as f64) * (CARD_W + GAP)
}

fn tableau_x(idx: usize) -> f64 {
    LEFT + tableau_spread() * idx as f64
}

fn tableau_spread() -> f64 {
    (BOARD_W - LEFT * 2.0 - CARD_W) / 6.0
}

fn tableau_card_pos(pile: &[Card], pile_idx: usize, card_idx: usize) -> (f64, f64) {
    let mut y = TABLEAU_TOP;
    for card in pile.iter().take(card_idx) {
        y += if card.face_up {
            FACE_UP_STEP
        } else {
            FACE_DOWN_STEP
        };
    }
    (tableau_x(pile_idx), y)
}

thread_local! {
    static GAME: RefCell<Game> = RefCell::new(Game::new(1));
    static RENDER: RefCell<Vec<RenderCard>> = const { RefCell::new(Vec::new()) };
}

#[unsafe(no_mangle)]
pub extern "C" fn new_game(seed: u32) {
    GAME.with(|game| {
        *game.borrow_mut() = Game::new(seed);
    });
    sync_render();
}

#[unsafe(no_mangle)]
pub extern "C" fn click(x: f64, y: f64) {
    GAME.with(|game| {
        game.borrow_mut().click(x, y);
    });
    sync_render();
}

#[unsafe(no_mangle)]
pub extern "C" fn auto_move_card(x: f64, y: f64) -> u8 {
    let moved = GAME.with(|game| game.borrow_mut().auto_move_card(x, y));
    sync_render();
    u8::from(moved)
}

#[unsafe(no_mangle)]
pub extern "C" fn auto_play_step() -> u8 {
    let moved = GAME.with(|game| game.borrow_mut().auto_play_step());
    sync_render();
    u8::from(moved)
}

#[unsafe(no_mangle)]
pub extern "C" fn undo() -> u8 {
    let undone = GAME.with(|game| game.borrow_mut().undo());
    sync_render();
    u8::from(undone)
}

#[unsafe(no_mangle)]
pub extern "C" fn can_undo() -> u8 {
    GAME.with(|game| u8::from(game.borrow().can_undo()))
}

#[unsafe(no_mangle)]
pub extern "C" fn sync_render() {
    let cards = GAME.with(|game| game.borrow().render_cards());
    RENDER.with(|render| {
        *render.borrow_mut() = cards;
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn render_count() -> usize {
    RENDER.with(|render| render.borrow().len())
}

#[unsafe(no_mangle)]
pub extern "C" fn card_x(idx: usize) -> f64 {
    render_card(idx).map_or(0.0, |card| card.x)
}

#[unsafe(no_mangle)]
pub extern "C" fn card_y(idx: usize) -> f64 {
    render_card(idx).map_or(0.0, |card| card.y)
}

#[unsafe(no_mangle)]
pub extern "C" fn card_rank(idx: usize) -> u8 {
    render_card(idx).map_or(0, |card| card.rank)
}

#[unsafe(no_mangle)]
pub extern "C" fn card_suit(idx: usize) -> u8 {
    render_card(idx).map_or(0, |card| card.suit)
}

#[unsafe(no_mangle)]
pub extern "C" fn card_face_up(idx: usize) -> u8 {
    render_card(idx).map_or(0, |card| u8::from(card.face_up))
}

#[unsafe(no_mangle)]
pub extern "C" fn card_selected(idx: usize) -> u8 {
    render_card(idx).map_or(0, |card| u8::from(card.selected))
}

#[unsafe(no_mangle)]
pub extern "C" fn stock_count() -> usize {
    GAME.with(|game| game.borrow().stock.len())
}

#[unsafe(no_mangle)]
pub extern "C" fn waste_count() -> usize {
    GAME.with(|game| game.borrow().waste.len())
}

#[unsafe(no_mangle)]
pub extern "C" fn moves_count() -> u32 {
    GAME.with(|game| game.borrow().moves)
}

#[unsafe(no_mangle)]
pub extern "C" fn score() -> i32 {
    GAME.with(|game| game.borrow().score)
}

#[unsafe(no_mangle)]
pub extern "C" fn won() -> u8 {
    GAME.with(|game| u8::from(game.borrow().won()))
}

#[unsafe(no_mangle)]
pub extern "C" fn layout_version() -> u32 {
    4
}

fn render_card(idx: usize) -> Option<RenderCard> {
    RENDER.with(|render| render.borrow().get(idx).copied())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_game_deals_expected_visible_cards() {
        let game = Game::new(42);
        assert_eq!(game.stock.len(), 24);
        assert_eq!(game.tableau.iter().map(Vec::len).sum::<usize>(), 28);
        assert_eq!(
            game.tableau
                .iter()
                .filter(|pile| pile.last().is_some_and(|card| card.face_up))
                .count(),
            7
        );
    }

    #[test]
    fn stock_click_deals_to_waste() {
        let mut game = Game::new(42);
        game.click(stock_x() + 4.0, TOP + 4.0);
        assert_eq!(game.stock.len(), 23);
        assert_eq!(game.waste.len(), 1);
        assert!(game.waste.last().unwrap().face_up);
    }

    #[test]
    fn undo_restores_stock_draw() {
        let mut game = Game::new(42);
        let top_stock = *game.stock.last().unwrap();

        game.click(stock_x() + 4.0, TOP + 4.0);
        assert!(game.can_undo());
        assert_eq!(game.moves, 1);

        assert!(game.undo());
        assert_eq!(game.moves, 0);
        assert_eq!(game.score, 0);
        assert_eq!(game.stock.len(), 24);
        assert_eq!(game.waste.len(), 0);
        assert_eq!(*game.stock.last().unwrap(), top_stock);
        assert!(!game.can_undo());
    }

    #[test]
    fn undo_restores_manual_tableau_flip() {
        let mut game = Game::new(42);
        let pile_idx = 1;
        let card_idx = game.tableau[pile_idx].len() - 1;
        game.tableau[pile_idx][card_idx].face_up = false;
        assert!(!game.tableau[pile_idx][card_idx].face_up);

        let (x, y) = tableau_card_pos(&game.tableau[pile_idx], pile_idx, card_idx);
        game.click(x + 4.0, y + 4.0);
        assert!(game.tableau[pile_idx][card_idx].face_up);
        assert_eq!(game.score, SCORE_TURN_TABLEAU);

        assert!(game.undo());
        assert!(!game.tableau[pile_idx][card_idx].face_up);
        assert_eq!(game.moves, 0);
        assert_eq!(game.score, 0);
    }

    #[test]
    fn new_game_starts_without_undo_history() {
        let mut game = Game::new(42);
        game.click(stock_x() + 4.0, TOP + 4.0);
        assert!(game.can_undo());

        game = Game::new(99);
        assert!(!game.can_undo());
        assert!(!game.undo());
    }

    #[test]
    fn scoring_tracks_common_card_moves() {
        let mut game = Game::new(42);
        game.waste.push(Card {
            suit: 0,
            rank: 1,
            face_up: true,
        });

        assert!(game.try_move_to_foundation(Location::Waste, 0));
        assert_eq!(game.score, SCORE_TO_FOUNDATION);

        game.tableau[0] = vec![Card {
            suit: 1,
            rank: 2,
            face_up: true,
        }];
        assert!(game.try_move_to_tableau(Location::Foundation(0), 0));
        assert_eq!(
            game.score,
            SCORE_TO_FOUNDATION + SCORE_FOUNDATION_TO_TABLEAU
        );

        game.tableau[1].clear();
        game.waste.push(Card {
            suit: 2,
            rank: 13,
            face_up: true,
        });
        assert!(game.try_move_to_tableau(Location::Waste, 1));
        assert_eq!(
            game.score,
            SCORE_TO_FOUNDATION + SCORE_FOUNDATION_TO_TABLEAU + SCORE_WASTE_TO_TABLEAU
        );

        game.stock.clear();
        game.waste.push(Card {
            suit: 1,
            rank: 7,
            face_up: true,
        });
        game.draw_stock();
        assert_eq!(
            game.score,
            SCORE_TO_FOUNDATION
                + SCORE_FOUNDATION_TO_TABLEAU
                + SCORE_WASTE_TO_TABLEAU
                + SCORE_RECYCLE_STOCK
        );
    }

    #[test]
    fn auto_move_sends_waste_card_to_foundation() {
        let mut game = Game::new(42);
        game.waste.push(Card {
            suit: 0,
            rank: 1,
            face_up: true,
        });

        assert!(game.auto_move_card(waste_x() + 4.0, TOP + 4.0));
        assert!(game.waste.is_empty());
        assert_eq!(game.foundations[0].len(), 1);
        assert_eq!(game.moves, 1);
        assert_eq!(game.score, SCORE_TO_FOUNDATION);

        assert!(game.undo());
        assert_eq!(game.waste.len(), 1);
        assert!(game.foundations[0].is_empty());
        assert_eq!(game.score, 0);
    }

    #[test]
    fn auto_move_only_uses_top_tableau_cards() {
        let mut game = Game::new(42);
        game.tableau[0] = vec![Card {
            suit: 0,
            rank: 1,
            face_up: true,
        }];
        let (x, y) = tableau_card_pos(&game.tableau[0], 0, 0);

        assert!(game.auto_move_card(x + 4.0, y + 4.0));
        assert!(game.tableau[0].is_empty());
        assert_eq!(game.foundations[0].len(), 1);

        let mut game = Game::new(42);
        for pile in &mut game.tableau {
            pile.clear();
        }
        game.tableau[1] = vec![
            Card {
                suit: 1,
                rank: 1,
                face_up: true,
            },
            Card {
                suit: 2,
                rank: 9,
                face_up: true,
            },
        ];
        let (lower_x, lower_y) = tableau_card_pos(&game.tableau[1], 1, 0);

        assert!(!game.auto_move_card(lower_x + 4.0, lower_y + 4.0));
        assert_eq!(game.tableau[1].len(), 2);
    }

    #[test]
    fn auto_move_sends_black_nine_to_red_ten() {
        let mut game = Game::new(42);
        game.tableau[0] = vec![Card {
            suit: 1,
            rank: 10,
            face_up: true,
        }];
        game.tableau[1] = vec![Card {
            suit: 0,
            rank: 9,
            face_up: true,
        }];
        let (x, y) = tableau_card_pos(&game.tableau[1], 1, 0);

        assert!(game.auto_move_card(x + 4.0, y + 4.0));
        assert_eq!(game.tableau[0].len(), 2);
        assert!(game.tableau[1].is_empty());
        assert_eq!(game.tableau[0][1].rank, 9);
        assert_eq!(game.moves, 1);
    }

    #[test]
    fn auto_play_prefers_foundation_move() {
        let mut game = Game::new(42);
        game.stock.clear();
        game.waste.push(Card {
            suit: 0,
            rank: 1,
            face_up: true,
        });

        assert!(game.auto_play_step());
        assert!(game.waste.is_empty());
        assert_eq!(game.foundations[0].len(), 1);
        assert_eq!(game.score, SCORE_TO_FOUNDATION);
    }

    #[test]
    fn auto_play_moves_waste_to_tableau() {
        let mut game = Game::new(42);
        game.stock.clear();
        for pile in &mut game.tableau {
            pile.clear();
        }
        game.waste.push(Card {
            suit: 0,
            rank: 9,
            face_up: true,
        });
        game.tableau[0] = vec![Card {
            suit: 1,
            rank: 10,
            face_up: true,
        }];

        assert!(game.auto_play_step());
        assert!(game.waste.is_empty());
        assert_eq!(game.tableau[0].len(), 2);
        assert_eq!(game.score, SCORE_WASTE_TO_TABLEAU);
    }

    #[test]
    fn auto_play_moves_tableau_stack_when_it_reveals_card() {
        let mut game = Game::new(42);
        game.stock.clear();
        for pile in &mut game.tableau {
            pile.clear();
        }
        game.tableau[0] = vec![Card {
            suit: 1,
            rank: 10,
            face_up: true,
        }];
        game.tableau[1] = vec![
            Card {
                suit: 2,
                rank: 4,
                face_up: false,
            },
            Card {
                suit: 0,
                rank: 9,
                face_up: true,
            },
        ];

        assert!(game.auto_play_step());
        assert_eq!(game.tableau[0].len(), 2);
        assert_eq!(game.tableau[1].len(), 1);
        assert!(game.tableau[1][0].face_up);
        assert_eq!(game.score, SCORE_TURN_TABLEAU);
    }

    #[test]
    fn auto_play_draws_stock_but_does_not_recycle() {
        let mut game = Game::new(42);
        for pile in &mut game.tableau {
            pile.clear();
        }
        game.waste.clear();
        game.stock = vec![Card {
            suit: 0,
            rank: 5,
            face_up: false,
        }];

        assert!(game.auto_play_step());
        assert_eq!(game.stock.len(), 0);
        assert_eq!(game.waste.len(), 1);
        assert_eq!(game.score, 0);

        assert!(!game.auto_play_step());
        assert_eq!(game.waste.len(), 1);
        assert_eq!(game.score, 0);
    }

    #[test]
    fn render_keeps_waste_card_under_drag_target() {
        let mut game = Game::new(42);
        game.click(stock_x() + 4.0, TOP + 4.0);
        game.click(stock_x() + 4.0, TOP + 4.0);

        let rendered = game.render_cards();
        let waste_cards = rendered
            .iter()
            .filter(|card| card.x == waste_x() && card.y == TOP)
            .count();

        assert_eq!(waste_cards, 2);
        assert!(!rendered[0].selected);
    }

    #[test]
    fn only_kings_can_move_to_empty_tableau() {
        let ace = Card {
            suit: 0,
            rank: 1,
            face_up: true,
        };
        let king = Card {
            suit: 1,
            rank: 13,
            face_up: true,
        };
        assert!(!can_place_on_tableau(ace, &[]));
        assert!(can_place_on_tableau(king, &[]));
    }

    #[test]
    fn tableau_uses_full_board_spread() {
        assert_eq!(tableau_x(0), LEFT);
        assert!(tableau_x(6) > 950.0);
    }
}
