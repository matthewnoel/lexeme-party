<script lang="ts">
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { onMount, onDestroy } from 'svelte';
	import {
		gs,
		connect,
		setOnWelcome,
		defaultWsUrl,
		type GameMode
	} from '$lib/game/connection.svelte';
	import { debugMode } from '$lib/debug';

	let wsUrl = $state('ws://localhost:4000/ws');
	let playerName = $state('');
	let roomCodeInput = $state('');
	let selectedGameMode = $state<GameMode>('keyboarding');
	let code = $derived(roomCodeInput.trim().toUpperCase());

	onMount(() => {
		wsUrl = defaultWsUrl();
		setOnWelcome((roomCode) => {
			goto(resolve(`/room/${roomCode}`));
		});
	});

	onDestroy(() => {
		setOnWelcome(null);
	});

	function createRoom(): void {
		connect(wsUrl, {
			playerName,
			gameMode: selectedGameMode
		});
	}

	function joinRoom(): void {
		if (!code) {
			gs.errorMessage = 'Enter a room code to join';
			return;
		}
		connect(wsUrl, {
			roomCode: code,
			playerName,
			gameMode: selectedGameMode
		});
	}
</script>

<main class="pregame">
	<h1>New Game</h1>
	<label>
		Your name (optional)
		<input
			bind:value={playerName}
			placeholder="Player name"
			autocomplete="off"
			autocorrect="off"
			autocapitalize="off"
			spellcheck="false"
		/>
	</label>
	<label>
		Game mode
		<select bind:value={selectedGameMode}>
			<option value="keyboarding">Keyboarding</option>
			<option value="arithmetic">Arithmetic</option>
		</select>
	</label>
	{#if debugMode}
		<label>
			Server URL
			<input
				bind:value={wsUrl}
				placeholder="ws://localhost:4000/ws"
				autocomplete="off"
				autocorrect="off"
				autocapitalize="off"
				spellcheck="false"
			/>
		</label>
	{/if}
	<label>
		Room code
		<input
			bind:value={roomCodeInput}
			placeholder="ABCD"
			maxlength="8"
			autocomplete="off"
			autocorrect="off"
			autocapitalize="off"
			spellcheck="false"
		/>
	</label>
	<div class="buttons">
		<button onclick={createRoom} disabled={gs.phase === 'connecting' || !!code}>Create room</button>
		<button onclick={joinRoom} disabled={gs.phase === 'connecting' || !code}>Join room</button>
	</div>
	{#if gs.errorMessage}
		<p class="error">{gs.errorMessage}</p>
	{/if}
	{#if debugMode}
		<p class="meta">socket: {gs.socketState}</p>
		{#if gs.lastSocketDetail}
			<p class="meta">{gs.lastSocketDetail}</p>
		{/if}
	{/if}
</main>

<style>
	h1 {
		text-align: center;
		margin: 1rem 0 0 0;
	}
	.pregame {
		max-width: 460px;
		margin: 0 auto;
		padding: 0.5rem 1.25rem;
		display: grid;
		gap: 0.75rem;
	}

	label {
		display: grid;
		gap: 0.25rem;
		font-size: 0.92rem;
	}

	input,
	select {
		padding: 0.6rem;
		color: inherit;
	}

	.buttons {
		display: flex;
		gap: 0.5rem;
	}

	button {
		padding: 0.55rem 0.8rem;
		color: inherit;
		cursor: pointer;
	}

	.meta {
		margin: 0;
		font-size: 0.8rem;
	}
</style>
