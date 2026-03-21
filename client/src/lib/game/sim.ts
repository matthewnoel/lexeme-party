import type { PlayerSnapshot } from './protocol';

export type BlobLayout = Record<number, { x: number; y: number }>;

function lerp(from: number, to: number, factor: number): number {
	return from + (to - from) * factor;
}

export function nextBlobLayout(
	players: PlayerSnapshot[],
	current: BlobLayout,
	elapsedMs: number,
	width: number,
	height: number
): BlobLayout {
	if (players.length === 0) {
		return {};
	}

	const centerX = width / 2;
	const centerY = height / 2;

	const ranked = [...players].sort((a, b) => b.size - a.size);
	const largest = ranked[0];
	const rotation = elapsedMs * 0.00035;
	const next: BlobLayout = {};

	for (let index = 0; index < ranked.length; index += 1) {
		const player = ranked[index];
		const previous = current[player.id] ?? { x: centerX, y: centerY };

		let targetX = centerX;
		let targetY = centerY;

		if (player.id !== largest.id) {
			const orbitIndex = index - 1;
			const orbitCount = Math.max(1, ranked.length - 1);
			const angle = rotation + (orbitIndex * Math.PI * 2) / orbitCount;
			const radius = 90 + orbitIndex * 26;

			targetX = centerX + Math.cos(angle) * radius;
			targetY = centerY + Math.sin(angle) * radius;
		} else {
			targetX = centerX + Math.cos(rotation * 1.3) * 8;
			targetY = centerY + Math.sin(rotation * 1.1) * 8;
		}

		next[player.id] = {
			x: lerp(previous.x, targetX, 0.08),
			y: lerp(previous.y, targetY, 0.08)
		};
	}

	return next;
}
