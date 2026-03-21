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
			Game mode
			<Select
				bind:value={selectedGameMode}
				options={[
					{ value: 'keyboarding', label: 'Keyboarding' },
					{ value: 'arithmetic', label: 'Arithmetic' }
				]}
			/>
		</label>
		<label>
			Your name (optional)
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
			Room code (optional)
			<TextInput
				bind:value={roomCodeInput}
				placeholder="ABCD"
				maxlength={8}
				autocomplete="off"
				autocorrect="off"
				autocapitalize="off"
				spellcheck="false"
			/>
		</label>
		<div class="buttons">
			<Button
				label="Create room"
				onclick={createRoom}
				disabled={gs.phase === 'connecting' || !!code}
			/>
			<Button label="Join room" onclick={joinRoom} disabled={gs.phase === 'connecting' || !code} />
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
		font-size: 2.5rem;
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
		padding: 0.5rem 1.25rem 7rem 1.25rem;
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
</style>
