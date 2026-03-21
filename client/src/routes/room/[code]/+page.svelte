<script lang="ts">
	import { page } from '$app/state';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { browser } from '$app/environment';
	import { onMount, onDestroy } from 'svelte';
	import {
		gs,
		connect,
		setOnDisconnect,
		handlePromptInput,
		submitPrompt,
		socketStateLabel,
		defaultWsUrl,
		loadSession,
		loadRejoinToken
	} from '$lib/game/connection.svelte';
	import { nextBlobLayout, type BlobLayout } from '$lib/game/sim';
	import type { PlayerSnapshot } from '$lib/game/protocol';

	let arenaEl: HTMLDivElement | null = $state(null);
	let blobLayout: BlobLayout = $state({});
	let debugOpen = $state(false);
	let animationHandle = 0;

	function animate(): void {
		if (gs.room && arenaEl) {
			blobLayout = nextBlobLayout(
				gs.room.players,
				blobLayout,
				performance.now(),
				arenaEl.clientWidth,
				arenaEl.clientHeight
			);
		}
		animationHandle = requestAnimationFrame(animate);
	}

	function circleSize(player: PlayerSnapshot): number {
		return Math.max(42, Math.min(220, player.size * 4));
	}

	onMount(() => {
		setOnDisconnect(() => goto(resolve('/')));

		if (gs.phase !== 'ingame') {
			const code = page.params.code ?? '';
			const session = loadSession();
			const rejoinToken = loadRejoinToken(code);
			connect(session?.wsUrl ?? defaultWsUrl(), {
				roomCode: code,
				playerName: session?.playerName,
				gameMode: session?.gameMode,
				rejoinToken: rejoinToken ?? undefined
			});
		}

		animationHandle = requestAnimationFrame(animate);
	});

	onDestroy(() => {
		if (browser) {
			cancelAnimationFrame(animationHandle);
		}
		setOnDisconnect(null);
	});
</script>

<main class="game">
	<header>
		<div class="prompt">{gs.room?.prompt ?? 'Waiting for prompt...'}</div>
		<input
			value={gs.promptInput}
			oninput={(e) => handlePromptInput(e.currentTarget.value)}
			onkeydown={(e) => {
				if (e.key === 'Enter') submitPrompt();
			}}
			placeholder="Type your answer, press Enter to submit"
			autocomplete="off"
			autocorrect="off"
			autocapitalize="off"
			spellcheck="false"
		/>
		{#if gs.latestRoundSummary}
			<div class="result">{gs.latestRoundSummary}</div>
		{/if}
	</header>
	<div class="arena" bind:this={arenaEl}>
		{#if gs.room}
			{#each gs.room.players as player (player.id)}
				<div
					class="blob {player.id === gs.playerId ? 'me' : ''}"
					style={`--blob-color:${player.color}; width:${circleSize(player)}px; height:${circleSize(player)}px; left:${(blobLayout[player.id]?.x ?? 0) - circleSize(player) / 2}px; top:${(blobLayout[player.id]?.y ?? 0) - circleSize(player) / 2}px;`}
				>
					<div class="name">{player.name}</div>
					<div class="size">{player.size.toFixed(1)}</div>
					<div class="progress">{player.progress}</div>
				</div>
			{/each}
		{/if}
	</div>
	<aside class="debug">
		<button onclick={() => (debugOpen = !debugOpen)}>
			{debugOpen ? 'Hide' : 'ℹ️ Stats for nerds'}
		</button>
		{#if debugOpen}
			<dl>
				<dt>game</dt>
				<dd>{gs.gameKey || 'unknown'}</dd>
				<dt>room</dt>
				<dd>{gs.room?.roomCode ?? '-'}</dd>
				<dt>socket</dt>
				<dd>{socketStateLabel()}</dd>
				<dt>inbound</dt>
				<dd>{gs.inboundCount}</dd>
				<dt>outbound</dt>
				<dd>{gs.outboundCount}</dd>
				<dt>players</dt>
				<dd>{gs.room?.players.length ?? 0}</dd>
				<dt>min eat size</dt>
				<dd>{gs.minEatableSize.toFixed(1)}</dd>
			</dl>
		{/if}
	</aside>
</main>

<style>
	main {
		min-height: 100vh;
	}

	.game {
		display: grid;
		grid-template-rows: auto 1fr;
	}

	header {
		padding: 0.75rem;
		display: grid;
		gap: 0.5rem;
		position: relative;
		z-index: 2;
	}

	input {
		padding: 0.6rem;
		color: inherit;
	}

	.prompt {
		font-size: 1.2rem;
		text-align: center;
	}

	.result {
		font-size: 0.9rem;
	}

	.arena {
		position: relative;
		overflow: hidden;
		min-height: 62vh;
	}

	.blob {
		position: absolute;
		background: var(--blob-color);
		border: 2px solid var(--blob-color);
		border-radius: 9999px;
		display: grid;
		place-content: center;
		gap: 0.2rem;
		text-align: center;
		padding: 0.5rem;
		box-sizing: border-box;
		transition:
			width 180ms linear,
			height 180ms linear;
	}

	.blob.me {
		outline: 2px solid;
	}

	.name {
		font-size: 0.85rem;
		font-weight: 600;
	}

	.size,
	.progress {
		font-size: 0.75rem;
	}

	.debug {
		display: flex;
		flex-direction: column;
		align-items: flex-end;
		position: fixed;
		right: 0.5rem;
		bottom: 0.5rem;
		border-radius: 0.4rem;
		padding: 0.5rem;
		width: 240px;
		z-index: 3;
	}

	button {
		padding: 0.55rem 0.8rem;
		color: inherit;
		cursor: pointer;
	}

	dl {
		display: grid;
		grid-template-columns: 1fr 1fr;
		gap: 0.15rem 0.4rem;
		margin: 0.45rem 0 0 0;
		font-size: 0.8rem;
	}

	dd {
		margin: 0;
		text-align: right;
	}
</style>
