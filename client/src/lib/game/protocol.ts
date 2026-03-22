export type PlayerSnapshot = {
	id: number;
	name: string;
	size: number;
	color: string;
	connected: boolean;
	progress: string;
};

export type RoomSnapshot = {
	roomCode: string;
	players: PlayerSnapshot[];
	prompt: string;
	roundId: number;
	matchWinner: number | null;
	matchRemainingMs: number | null;
	hostPlayerId: number;
};

export type ClientMessage =
	| {
			type: 'joinOrCreateRoom';
			playerName?: string;
			roomCode?: string;
			gameMode?: string;
			matchDurationSecs?: number;
	  }
	| { type: 'rejoinRoom'; rejoinToken: string }
	| { type: 'inputUpdate'; text: string }
	| { type: 'submitAttempt'; text: string }
	| { type: 'startMatch' }
	| { type: 'rematch' };

export type ServerMessage =
	| {
			type: 'welcome';
			playerId: number;
			roomCode: string;
			gameKey: string;
			inputPlaceholder: string;
			rejoinToken: string;
	  }
	| { type: 'roomState'; room: RoomSnapshot }
	| { type: 'promptState'; roomCode: string; roundId: number; prompt: string }
	| { type: 'raceProgress'; roomCode: string; playerId: number; text: string }
	| {
			type: 'roundResult';
			roomCode: string;
			roundId: number;
			winnerPlayerId: number;
			growthAwarded: number;
	  }
	| { type: 'wrongAnswer'; roomCode: string; playerId: number; shrinkApplied: number }
	| { type: 'error'; message: string };

function isObject(value: unknown): value is Record<string, unknown> {
	return typeof value === 'object' && value !== null;
}

function isPlayerSnapshot(value: unknown): value is PlayerSnapshot {
	if (!isObject(value)) return false;
	return (
		typeof value.id === 'number' &&
		typeof value.name === 'string' &&
		typeof value.size === 'number' &&
		typeof value.color === 'string' &&
		typeof value.connected === 'boolean' &&
		typeof value.progress === 'string'
	);
}

function isRoomSnapshot(value: unknown): value is RoomSnapshot {
	if (!isObject(value) || !Array.isArray(value.players)) return false;
	return (
		typeof value.roomCode === 'string' &&
		typeof value.prompt === 'string' &&
		typeof value.roundId === 'number' &&
		(value.matchWinner === null || typeof value.matchWinner === 'number') &&
		(value.matchRemainingMs === null || typeof value.matchRemainingMs === 'number') &&
		typeof value.hostPlayerId === 'number' &&
		value.players.every(isPlayerSnapshot)
	);
}

function isServerMessage(value: unknown): value is ServerMessage {
	if (!isObject(value) || typeof value.type !== 'string') {
		return false;
	}
	switch (value.type) {
		case 'welcome':
			return (
				typeof value.playerId === 'number' &&
				typeof value.roomCode === 'string' &&
				typeof value.gameKey === 'string' &&
				typeof value.inputPlaceholder === 'string' &&
				typeof value.rejoinToken === 'string'
			);
		case 'roomState':
			return isRoomSnapshot(value.room);
		case 'promptState':
			return (
				typeof value.roomCode === 'string' &&
				typeof value.roundId === 'number' &&
				typeof value.prompt === 'string'
			);
		case 'raceProgress':
			return (
				typeof value.roomCode === 'string' &&
				typeof value.playerId === 'number' &&
				typeof value.text === 'string'
			);
		case 'roundResult':
			return (
				typeof value.roomCode === 'string' &&
				typeof value.roundId === 'number' &&
				typeof value.winnerPlayerId === 'number' &&
				typeof value.growthAwarded === 'number'
			);
		case 'wrongAnswer':
			return (
				typeof value.roomCode === 'string' &&
				typeof value.playerId === 'number' &&
				typeof value.shrinkApplied === 'number'
			);
		case 'error':
			return typeof value.message === 'string';
		default:
			return false;
	}
}

export function decodeServerMessage(raw: string): ServerMessage | null {
	try {
		const parsed: unknown = JSON.parse(raw);
		return isServerMessage(parsed) ? parsed : null;
	} catch {
		return null;
	}
}
