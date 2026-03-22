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
	import Button from '$lib/components/Button.svelte';
	import Select from '$lib/components/Select.svelte';
	import TextInput from '$lib/components/TextInput.svelte';

	let wsUrl = $state('ws://localhost:4000/ws');
	let playerName = $state('');
	let roomCodeInput = $state('');
	let selectedGameMode = $state<GameMode>('keyboarding');
	let matchDuration = $state('60');
	let code = $derived(roomCodeInput);

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
			gameMode: selectedGameMode,
			matchDurationSecs: parseInt(matchDuration) || 60
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

<main>
	<div class="pregame">
		<h1 class="shizuru-regular">edif.io</h1>
		{#if debugMode}
			<label>
				Server URL
				<TextInput
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
			<strong>Game Mode:</strong>
			<Select
				bind:value={selectedGameMode}
				options={[
					{ value: 'keyboarding', label: 'Keyboarding' },
					{ value: 'arithmetic', label: 'Arithmetic' }
				]}
			/>
		</label>
		<label>
			<strong>Match Duration in Seconds:</strong>
			<TextInput
				bind:value={matchDuration}
				type="number"
				min="5"
				placeholder="60"
				autocomplete="off"
			/>
		</label>
		<label>
			<strong>Your Name (optional):</strong>
			<TextInput
				bind:value={playerName}
				placeholder="Player name"
				autocomplete="off"
				autocorrect="off"
				autocapitalize="off"
				spellcheck="false"
			/>
		</label>
		<label>
			<strong>Room Code (optional):</strong>
			<TextInput
				value={roomCodeInput}
				oninput={(e) => {
					const el = e.currentTarget;
					el.value = el.value.replace(/[^a-zA-Z]/g, '').toUpperCase();
					roomCodeInput = el.value;
				}}
				placeholder="ABCD"
				maxlength={4}
				pattern={'[A-Z]{4}'}
				autocomplete="off"
				autocorrect="off"
				autocapitalize="characters"
				spellcheck="false"
			/>
		</label>
		<div class="buttons">
			<Button
				label="Create Room"
				onclick={createRoom}
				disabled={gs.phase === 'connecting' || !!code}
			/>
			<Button label="Join Room" onclick={joinRoom} disabled={gs.phase === 'connecting' || !code} />
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
	</div>
</main>

<style>
	h1 {
		font-size: 4rem;
		text-align: center;
		margin: 1rem 0 0 0;
	}
	main {
		height: 100vh;
		display: flex;
		flex-direction: column;
		justify-content: center;
		align-items: stretch;
	}
	.pregame {
		width: 100%;
		max-width: 460px;
		margin: 0 auto;
		padding: 0.5rem 1.25rem 10rem 1.25rem;
		display: grid;
		gap: 0.75rem;
	}

	label {
		display: grid;
		gap: 0.25rem;
		font-size: 0.92rem;
	}

	.buttons {
		display: flex;
		gap: 0.5rem;
	}

	.meta {
		margin: 0;
		font-size: 0.8rem;
	}
	.error {
		background-color: transparent;
		color: red;
		padding: 0.5rem;
		border: 2px solid red;
		border-radius: 0.5rem;
		font-size: 0.8rem;
		margin: 0;
	}
</style>
