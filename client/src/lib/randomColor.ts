// match the palette used in /core/src/server.rs
const PALETTE = ['#38bdf8', '#a78bfa', '#34d399', '#f472b6', '#fbbf24', '#fb7185', '#22d3ee'];

export function randomColor(): string {
	const idx = Math.floor(Math.random() * PALETTE.length);
	return PALETTE[idx];
}
