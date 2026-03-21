import { describe, expect, it } from 'vitest';
import { nextBlobLayout } from './sim';

describe('nextBlobLayout', () => {
	it('returns empty layout for empty players', () => {
		expect(nextBlobLayout([], {}, 0, 800, 600)).toEqual({});
	});

	it('provides coordinates for each player', () => {
		const players = [
			{ id: 1, name: 'A', size: 20, color: '#fff', connected: true, progress: '' },
			{ id: 2, name: 'B', size: 10, color: '#000', connected: true, progress: '' }
		];
		const next = nextBlobLayout(players, {}, 16, 800, 600);
		expect(Object.keys(next)).toHaveLength(2);
		expect(next[1].x).toBeTypeOf('number');
		expect(next[2].y).toBeTypeOf('number');
	});
});
