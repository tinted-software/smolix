use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::visit::{Bfs, Dfs, DfsPostOrder, Topo};
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, Instant};
use std::{collections::BTreeMap, path::PathBuf};

use ratatui::backend::{Backend, CrosstermBackend};
use ratatui::crossterm::event::{Event, KeyCode, KeyModifiers, MouseEventKind};
use ratatui::layout::{Constraint, Layout, Position, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Span;
use ratatui::widgets::{
	Block, Borders, ListItem, Scrollbar, ScrollbarOrientation, Widget,
};
use ratatui::{crossterm, Frame, Terminal};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct InputDerivation {
	pub outputs: Vec<String>,
	#[serde(rename = "dynamicOutputs")]
	pub dynamic_outputs: BTreeMap<String, String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct StoreDerivationOutput {
	pub path: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct StoreDerivation {
	pub args: Vec<String>,
	pub builder: String,
	pub env: BTreeMap<String, String>,
	#[serde(rename = "inputDrvs")]
	pub input_derivations: BTreeMap<String, InputDerivation>,
	#[serde(rename = "inputSrcs")]
	pub input_sources: Vec<String>,
	pub name: String,
	pub outputs: BTreeMap<String, StoreDerivationOutput>,
	pub system: String,
}

#[derive(Default)]
pub struct DerivationBuilder {
	pub graph: DiGraph<StoreDerivation, ()>,
}

impl DerivationBuilder {
	pub fn add_derivation(
		&mut self,
		derivation: StoreDerivation,
	) -> NodeIndex<u32> {
		let node = self.graph.add_node(derivation.clone());

		for input_derivation in &derivation.input_derivations {
			let drv_file = std::fs::File::open(input_derivation.0).unwrap();
			let store_derivation: StoreDerivation =
				serde_json::from_reader(drv_file).unwrap();
			if let None = self
				.graph
				.raw_nodes()
				.iter()
				.position(|x| x.weight.name == store_derivation.name)
			{
				tracing::debug!(
					"Adding input derivation {}",
					store_derivation.name
				);

				let input_node = self.add_derivation(store_derivation.clone());

				self.graph.add_edge(node, input_node, ());
			};
		}

		node
	}
}

pub enum Direction {
	Up,
	Down,
}

#[derive(Clone)]
pub struct Tree<'a> {
	graph: &'a DiGraph<StoreDerivation, ()>,
	root: NodeIndex,
	style: Style,
	scroll: usize,
	selected: Option<NodeIndex>,
	expanded: HashMap<NodeIndex, bool>,
	visible_nodes: Vec<NodeIndex>,
	height: usize,
}

impl<'a> Tree<'a> {
	pub fn new(
		graph: &'a DiGraph<StoreDerivation, ()>,
		root: NodeIndex,
	) -> Self {
		Self {
			graph,
			root,
			style: Style::default(),
			scroll: 0,
			height: 0,
			selected: None,
			expanded: Default::default(),
			visible_nodes: graph.node_indices().collect(),
		}
	}

	pub fn style(mut self, style: Style) -> Self {
		self.style = style;
		self
	}

	fn render_node(
		&self,
		node: NodeIndex,
		depth: usize,
		expanded: &mut HashMap<NodeIndex, bool>,
	) -> Vec<Span> {
		let mut result = vec![];
		let node_label = self.graph.node_weight(node).unwrap().name.clone();
		let prefix = "  ".repeat(depth);
		let is_expanded = expanded.entry(node).or_insert(true);

		let icon = if self.graph.neighbors(node).count() > 0 {
			if *is_expanded {
				"▼ "
			} else {
				"▶ "
			}
		} else {
			"  "
		};

		result.push(Span::styled(
			format!("{} {} {}", prefix, icon, node_label),
			self.style,
		));

		if *is_expanded {
			for child in self.graph.neighbors(node) {
				result.extend(self.render_node(child, depth + 1, expanded));
			}
		}

		result
	}

	pub fn set_height(&mut self, height: usize) {
		self.height = height;
	}

	pub fn scroll_up(&mut self) {
		if self.scroll > 0 {
			self.scroll = self.scroll.saturating_sub(1);
		}
	}

	pub fn scroll_down(&mut self) {
		let max_scroll = self.visible_nodes.len().saturating_sub(self.height);
		if self.scroll < max_scroll {
			self.scroll = (self.scroll + 1).min(max_scroll);
		}
	}

	pub fn move_selection(&mut self, direction: Direction) {
		self.collect_visible_nodes(self.root, 0);

		if let Some(current_index) = self
			.selected
			.and_then(|s| self.visible_nodes.iter().position(|&n| n == s))
		{
			let new_index = match direction {
				Direction::Up if current_index > 0 => current_index - 1,
				Direction::Down
					if current_index < self.visible_nodes.len() - 1 =>
				{
					current_index + 1
				}
				_ => current_index,
			};
			self.selected = Some(self.visible_nodes[new_index]);
			self.adjust_scroll(new_index);
		} else if !self.visible_nodes.is_empty() {
			self.selected = Some(self.visible_nodes[0]);
			self.adjust_scroll(0);
		}
	}

	fn adjust_scroll(&mut self, selected_index: usize) {
		let max_scroll = self.visible_nodes.len().saturating_sub(self.height);

		if selected_index < self.scroll {
			// Scroll up to show the selected item
			self.scroll = selected_index;
		} else if selected_index >= self.scroll + self.height.saturating_sub(1)
		{
			// Scroll down to show the selected item
			self.scroll =
				selected_index.saturating_sub(self.height.saturating_sub(2));
		}

		self.scroll = self.scroll.min(max_scroll);
	}

	pub fn toggle_selected(&mut self) {
		if let Some(selected) = self.selected {
			let is_expanded = self.expanded.entry(selected).or_insert(true);
			*is_expanded = !*is_expanded;
		}
	}

	fn collect_visible_nodes(&mut self, node: NodeIndex, depth: usize) {
		self.visible_nodes.push(node);
		if *self.expanded.get(&node).unwrap_or(&false) {
			for child in self.graph.neighbors(node) {
				self.collect_visible_nodes(child, depth + 1);
			}
		}
	}
}

impl<'a> Widget for Tree<'a> {
	fn render(self, area: Rect, buf: &mut ratatui::buffer::Buffer) {
		let mut expanded = HashMap::new();
		let items = self.render_node(self.root, 0, &mut expanded);

		let block = Block::default()
			.borders(Borders::ALL)
			.title("Derivation Tree")
			.style(self.style);

		let inner_area = block.inner(area);
		block.render(area, buf);

		for (i, spans) in items
			.into_iter()
			.enumerate()
			.skip(self.scroll)
			.take(inner_area.height as usize)
		{
			let y = inner_area.top() + i as u16 - self.scroll as u16;

			let style = if Some(self.visible_nodes[i]) == self.selected {
				self.style.add_modifier(Modifier::REVERSED)
			} else {
				self.style
			};
			buf.set_span(
				inner_area.left(),
				y,
				&spans.style(style),
				inner_area.width,
			);
		}
	}
}

fn main() -> std::io::Result<()> {
	tracing_subscriber::fmt::init();

	let drv_file = std::fs::File::open(
		"/nix/store/k2jx57zfyc51xk9zkdvb66l9a8k5n9f3-hello-2.12.1.drv.json",
	)
	.unwrap();
	let store_derivation: StoreDerivation =
		serde_json::from_reader(drv_file).unwrap();

	let mut derivation_builder = DerivationBuilder::default();
	let root_derivation = derivation_builder.add_derivation(store_derivation);

	/*

		let ordered_deps_rev =
		petgraph::algo::toposort(&derivation_builder.graph, None).unwrap();

	for node in ordered_deps_rev.iter().rev() {
		tracing::info!(
			"Building derivation {}",
			derivation_builder.graph[*node].name
		);
	}
	*/

	// Terminal initialization
	crossterm::terminal::enable_raw_mode()?;
	let mut stdout = std::io::stdout();
	crossterm::execute!(
		stdout,
		crossterm::terminal::EnterAlternateScreen,
		crossterm::event::EnableMouseCapture
	)?;
	let mut terminal = Terminal::new(CrosstermBackend::new(stdout))?;

	// App

	let res =
		run_app(&mut terminal, &derivation_builder.graph, root_derivation);

	// restore terminal
	crossterm::terminal::disable_raw_mode()?;
	crossterm::execute!(
		terminal.backend_mut(),
		crossterm::terminal::LeaveAlternateScreen,
		crossterm::event::DisableMouseCapture
	)?;
	terminal.show_cursor()?;

	if let Err(err) = res {
		println!("{err:?}");
	}

	Ok(())
}

fn run_app<B: Backend>(
	terminal: &mut Terminal<B>,
	graph: &DiGraph<StoreDerivation, ()>,
	root_node: NodeIndex<u32>,
) -> std::io::Result<()> {
	let mut tree =
		Tree::new(graph, root_node).style(Style::default().fg(Color::White));

	loop {
		let update = match crossterm::event::read()? {
			Event::Key(key) => match key.code {
				KeyCode::Up => {
					tree.move_selection(Direction::Up);
					true
				}
				KeyCode::Down => {
					tree.move_selection(Direction::Down);
					true
				}
				KeyCode::Enter => {
					tree.toggle_selected();
					true
				}
				KeyCode::Char('q') => return Ok(()),
				_ => false,
			},
			Event::Mouse(mouse) => match mouse.kind {
				MouseEventKind::ScrollUp => {
					tree.scroll_up();
					true
				}
				MouseEventKind::ScrollDown => {
					tree.scroll_down();
					true
				}
				MouseEventKind::Down(_) => {
					// Handle mouse clicks (you'd need to calculate which node was clicked)
					false
				}
				_ => false,
			},
			_ => false,
		};

		terminal.draw(|frame| {
			let chunks = Layout::default()
				.direction(ratatui::layout::Direction::Vertical)
				.constraints([Constraint::Percentage(100)].as_ref())
				.split(frame.area());

			tree.set_height(chunks[0].height as usize - 2);

			frame.render_widget(tree.clone(), chunks[0]);
		})?;
	}
}
