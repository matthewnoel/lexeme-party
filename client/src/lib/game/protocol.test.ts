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
					hostPlayerId: 1,
					activePowerups: []
				}
			})
		);
		expect(parsed?.type).toBe('roomState');
	});

	it('parses roomState with active powerups', () => {
		const parsed = decodeServerMessage(
			JSON.stringify({
				type: 'roomState',
				room: {
					roomCode: 'ABCD',
					players: [],
					prompt: '',
					roundId: 0,
					matchWinner: null,
					matchRemainingMs: null,
					hostPlayerId: 1,
					activePowerups: [
						{ kind: 'freezeAllCompetitors', sourcePlayerId: 2, remainingMs: 10000 },
						{ kind: 'doublePoints', sourcePlayerId: 3, remainingMs: 25000 }
					]
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

	it('parses powerUpOffered', () => {
		const parsed = decodeServerMessage(
			JSON.stringify({
				type: 'powerUpOffered',
				kind: 'freezeAllCompetitors',
				expiresInMs: 30000
			})
		);
		expect(parsed?.type).toBe('powerUpOffered');
	});

	it('parses powerUpActivated', () => {
		const parsed = decodeServerMessage(
			JSON.stringify({
				type: 'powerUpActivated',
				playerId: 2,
				kind: 'doublePoints',
				durationMs: 30000
			})
		);
		expect(parsed?.type).toBe('powerUpActivated');
	});

	it('parses powerUpOfferExpired', () => {
		const parsed = decodeServerMessage(
			JSON.stringify({
				type: 'powerUpOfferExpired',
				kind: 'doublePoints'
			})
		);
		expect(parsed?.type).toBe('powerUpOfferExpired');
	});

	it('parses powerUpEffectEnded', () => {
		const parsed = decodeServerMessage(
			JSON.stringify({
				type: 'powerUpEffectEnded',
				playerId: 1,
				kind: 'freezeAllCompetitors'
			})
		);
		expect(parsed?.type).toBe('powerUpEffectEnded');
	});

	it('rejects powerUpOffered with invalid kind', () => {
		const parsed = decodeServerMessage(
			JSON.stringify({
				type: 'powerUpOffered',
				kind: 'unknownPowerUp',
				expiresInMs: 30000
			})
		);
		expect(parsed).toBeNull();
	});
});
