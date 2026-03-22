import { browser } from '$app/environment';
import {
	decodeServerMessage,
	type ClientMessage,
	type PowerUpKind,
	type RoomSnapshot,
	type ServerMessage
} from './protocol';

export type ConnectionPhase = 'pregame' | 'connecting' | 'ingame';
export type GameMode = 'keyboarding' | 'arithmetic';

const SESSION_KEY = 'edifio-connection';
const REJOIN_PREFIX = 'edifio-rejoin-';

type SessionData = {
	playerName: string;
	gameMode: GameMode;
	wsUrl: string;
};

function saveSession(data: SessionData): void {
	if (browser) {
		sessionStorage.setItem(SESSION_KEY, JSON.stringify(data));
	}
}

export function loadSession(): SessionData | null {
	if (!browser) return null;
	try {
		const raw = sessionStorage.getItem(SESSION_KEY);
		return raw ? (JSON.parse(raw) as SessionData) : null;
	} catch {
		return null;
	}
}

function saveRejoinToken(roomCode: string, token: string): void {
	if (browser) {
		sessionStorage.setItem(REJOIN_PREFIX + roomCode, token);
	}
}

export function loadRejoinToken(roomCode: string): string | null {
	if (!browser) return null;
	return sessionStorage.getItem(REJOIN_PREFIX + roomCode);
}

export function defaultWsUrl(): string {
	if (!browser) return 'ws://localhost:4000/ws';
	const wsProtocol = location.protocol === 'https:' ? 'wss:' : 'ws:';
	return `${wsProtocol}//${location.host}/ws`;
}

function normalizeRoomCode(value: string): string {
	return value.trim().toUpperCase();
}

export type PendingPowerUp = { kind: PowerUpKind; expiresAt: number };

export const gs = $state({
	phase: 'pregame' as ConnectionPhase,
	playerId: null as number | null,
	room: null as RoomSnapshot | null,
	roomCode: '',
	gameKey: '',
	inputPlaceholder: '',
	promptInput: '',
	latestRoundSummary: '',
	latestRoundSummaryColor: '',
	errorMessage: '',
	inboundCount: 0,
	outboundCount: 0,
	socketState: 'idle',
	lastSocketDetail: '',
	pendingPowerUps: [] as PendingPowerUp[]
});

let socket: WebSocket | null = null;
let welcomeCallback: ((roomCode: string) => void) | null = null;
let disconnectCallback: (() => void) | null = null;

export function setOnWelcome(fn: ((roomCode: string) => void) | null): void {
	welcomeCallback = fn;
}

export function setOnDisconnect(fn: (() => void) | null): void {
	disconnectCallback = fn;
}

export function sendClientMessage(message: ClientMessage): void {
	if (!socket || socket.readyState !== WebSocket.OPEN) return;
	gs.outboundCount += 1;
	socket.send(JSON.stringify(message));
}

function handleServerMessage(message: ServerMessage): void {
	switch (message.type) {
		case 'welcome':
			gs.playerId = message.playerId;
			gs.gameKey = message.gameKey;
			gs.inputPlaceholder = message.inputPlaceholder;
			gs.roomCode = message.roomCode;
			gs.phase = 'ingame';
			saveRejoinToken(message.roomCode, message.rejoinToken);
			welcomeCallback?.(message.roomCode);
			break;
		case 'roomState':
			gs.room = message.room;
			if (message.room.matchWinner) {
				const winner = message.room.players.find((p) => p.id === message.room.matchWinner);
				gs.latestRoundSummary = `${winner?.name ?? `Player ${message.room.matchWinner}`} wins the match`;
				gs.latestRoundSummaryColor = winner?.color ?? '';
			}
			break;
		case 'promptState':
			if (gs.room) {
				gs.room = {
					...gs.room,
					prompt: message.prompt,
					roundId: message.roundId,
					players: gs.room.players.map((p) => ({ ...p, progress: '' }))
				};
			}
			gs.promptInput = '';
			break;
		case 'raceProgress':
			if (!gs.room) break;
			gs.room = {
				...gs.room,
				players: gs.room.players.map((p) =>
					p.id === message.playerId ? { ...p, progress: message.text } : p
				)
			};
			break;
		case 'roundResult':
			if (!gs.room) break;
			{
				const winner = gs.room.players.find((p) => p.id === message.winnerPlayerId);
				gs.latestRoundSummary = `${winner?.name ?? `Player ${message.winnerPlayerId}`} won +${message.growthAwarded.toFixed(1)} size`;
				gs.latestRoundSummaryColor = winner?.color ?? '';
				gs.promptInput = '';
			}
			break;
		case 'wrongAnswer':
			if (!gs.room) break;
			{
				const isMe = message.playerId === gs.playerId;
				if (isMe) {
					gs.latestRoundSummary = `Wrong! -${message.shrinkApplied.toFixed(1)} size`;
					gs.latestRoundSummaryColor = '#e74c3c';
					gs.promptInput = '';
				} else {
					const player = gs.room.players.find((p) => p.id === message.playerId);
					gs.latestRoundSummary = `${player?.name ?? `Player ${message.playerId}`} lost -${message.shrinkApplied.toFixed(1)} size`;
					gs.latestRoundSummaryColor = player?.color ?? '';
				}
			}
			break;
		case 'error':
			gs.errorMessage = message.message;
			break;
		case 'powerUpOffered':
			gs.pendingPowerUps = [
				...gs.pendingPowerUps,
				{ kind: message.kind, expiresAt: performance.now() + message.expiresInMs }
			];
			break;
		case 'powerUpOfferExpired': {
			const idx = gs.pendingPowerUps.findIndex((pu) => pu.kind === message.kind);
			if (idx !== -1) {
				gs.pendingPowerUps = gs.pendingPowerUps.toSpliced(idx, 1);
			}
			break;
		}
		case 'powerUpActivated':
			if (message.playerId === gs.playerId) {
				const idx = gs.pendingPowerUps.findIndex((pu) => pu.kind === message.kind);
				if (idx !== -1) {
					gs.pendingPowerUps = gs.pendingPowerUps.toSpliced(idx, 1);
				}
			}
			break;
		case 'powerUpEffectEnded':
			break;
	}
}

export function connect(
	wsUrl: string,
	opts?: {
		roomCode?: string;
		playerName?: string;
		gameMode?: GameMode;
		matchDurationSecs?: number;
		rejoinToken?: string;
	}
): void {
	if (gs.phase === 'connecting') return;

	gs.errorMessage = '';
	gs.latestRoundSummary = '';
	gs.latestRoundSummaryColor = '';
	gs.phase = 'connecting';
	gs.socketState = 'connecting';
	gs.lastSocketDetail = '';
	socket?.close();

	saveSession({
		playerName: opts?.playerName ?? '',
		gameMode: opts?.gameMode ?? 'keyboarding',
		wsUrl
	});

	try {
		socket = new WebSocket(wsUrl);
	} catch {
		gs.errorMessage = `Invalid WebSocket URL: ${wsUrl}`;
		gs.phase = 'pregame';
		return;
	}

	socket.onopen = () => {
		gs.socketState = 'open';
		if (opts?.rejoinToken) {
			sendClientMessage({ type: 'rejoinRoom', rejoinToken: opts.rejoinToken });
		} else {
			sendClientMessage({
				type: 'joinOrCreateRoom',
				playerName: opts?.playerName?.trim() || undefined,
				roomCode: opts?.roomCode ? normalizeRoomCode(opts.roomCode) : undefined,
				gameMode: opts?.gameMode,
				matchDurationSecs: opts?.matchDurationSecs
			});
		}
	};

	socket.onmessage = (event: MessageEvent) => {
		const decoded = decodeServerMessage(String(event.data));
		if (!decoded) return;
		gs.inboundCount += 1;
		handleServerMessage(decoded);
	};

	socket.onerror = () => {
		gs.errorMessage = 'WebSocket error';
		gs.lastSocketDetail = 'socket error event fired';
	};

	socket.onclose = (event: CloseEvent) => {
		gs.socketState = 'closed';
		gs.lastSocketDetail = `closed code=${event.code} reason=${event.reason || '(none)'}`;
		const wasActive = gs.phase !== 'pregame';
		if (wasActive) {
			gs.errorMessage = 'Disconnected from server';
		}
		gs.phase = 'pregame';
		gs.room = null;
		if (wasActive) {
			disconnectCallback?.();
		}
	};
}

export function disconnect(): void {
	socket?.close();
	socket = null;
	gs.phase = 'pregame';
	gs.room = null;
}

export function socketStateLabel(): string {
	if (!socket) return 'closed';
	if (socket.readyState === WebSocket.OPEN) return 'open';
	if (socket.readyState === WebSocket.CONNECTING) return 'connecting';
	if (socket.readyState === WebSocket.CLOSING) return 'closing';
	return 'closed';
}

export function handlePromptInput(value: string): void {
	gs.promptInput = value;
	sendClientMessage({ type: 'inputUpdate', text: value });
}

export function submitPrompt(): void {
	sendClientMessage({ type: 'submitAttempt', text: gs.promptInput });
}

export function startMatch(): void {
	sendClientMessage({ type: 'startMatch' });
}

export function rematch(): void {
	gs.latestRoundSummary = '';
	gs.latestRoundSummaryColor = '';
	gs.promptInput = '';
	gs.pendingPowerUps = [];
	sendClientMessage({ type: 'rematch' });
}
