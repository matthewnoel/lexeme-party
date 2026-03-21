export function randomColor(): string {
	const h = Math.floor(Math.random() * 360);
	const s = 80 + Math.floor(Math.random() * 21);
	const l = 45 + Math.floor(Math.random() * 16);
	return `hsl(${h}, ${s}%, ${l}%)`;
}
