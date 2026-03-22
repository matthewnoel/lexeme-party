import { describe, expect, it } from 'vitest';
import { decodeServerMessage } from './protocol';

describe('decodeServerMessage', () => {
	it('returns null for invalid json', () => {
		expect(decodeServerMessage('bad-json')).toBeNull();
	});

	it('parses welcome', () => {
		const parsed = decodeServerMessage(
			JSON.stringify({
				type: 'welcome',
				playerId: 4,
				roomCode: 'ABCD',
				gameKey: 'keyboarding',
				inputPlaceholder: 'Type here...',
				rejoinToken: 'abc123'
			})
		);
		expect(parsed?.type).toBe('welcome');
	});

	it('parses roomState', () => {
		const parsed = decodeServerMessage(
			JSON.stringify({
				type: 'roomState',
				room: {
					roomCode: 'ABCD',
					players: [
						{
							id: 1,
							name: 'Alice',
							size: 14.2,
							color: '#38bdf8',
							connected: true,
							progress: 'he'
						}
					],
					prompt: 'hello',
					roundId: 1,
					matchWinner: null,
					matchRemainingMs: 45000,
					hostPlayerId: 1
				}
			})
		);
		expect(parsed?.type).toBe('roomState');
	});

	it('parses promptState', () => {
		const parsed = decodeServerMessage(
			JSON.stringify({
				type: 'promptState',
				roomCode: 'ABCD',
				roundId: 2,
				prompt: 'world'
			})
		);
		expect(parsed?.type).toBe('promptState');
	});

	it('parses raceProgress', () => {
		const parsed = decodeServerMessage(
			JSON.stringify({
				type: 'raceProgress',
				roomCode: 'ABCD',
				playerId: 1,
				text: 'wo'
			})
		);
		expect(parsed?.type).toBe('raceProgress');
	});

	it('parses roundResult', () => {
		const parsed = decodeServerMessage(
			JSON.stringify({
				type: 'roundResult',
				roomCode: 'ABCD',
				roundId: 2,
				winnerPlayerId: 1,
				growthAwarded: 4
			})
		);
		expect(parsed?.type).toBe('roundResult');
	});

	it('parses error', () => {
		const parsed = decodeServerMessage(
			JSON.stringify({
				type: 'error',
				message: 'boom'
			})
		);
		expect(parsed?.type).toBe('error');
	});

	it('returns null for unknown message type', () => {
		const parsed = decodeServerMessage(
			JSON.stringify({
				type: 'pong',
				sentAtMs: 1,
				serverTimeMs: 2
			})
		);
		expect(parsed).toBeNull();
	});

	it('returns null for malformed known type payload', () => {
		const parsed = decodeServerMessage(
			JSON.stringify({
				type: 'roomState',
				room: { roomCode: 'ABCD' }
			})
		);
		expect(parsed).toBeNull();
	});
});
