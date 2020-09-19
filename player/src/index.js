'use strict'

const IPFS = require('ipfs');
const Hls = require('hls.js');
const HlsjsIpfsLoader = require('hlsjs-ipfs-loader');

document.addEventListener('DOMContentLoaded', async () => {
    console.log('Document loaded');

    const options = {
        // Preload is not helpful right now since the ref list is so large
        preload: { enabled: false },

        config: {
            Addresses: { Delegates: [] },
            Bootstrap: []
        }
    };

    // Any videos on the page might want to request network peers
    for (const video of document.getElementsByTagName('video'))
    {
        const delegates = video.dataset.ipfsDelegates;
        if (delegates) {
            Array.prototype.push.apply(options.config.Addresses.Delegates, delegates.split(' '));
        }

        const bootstrap = video.dataset.ipfsBootstrap;
        if (bootstrap) {
            Array.prototype.push.apply(options.config.Bootstrap, bootstrap.split(' '));
        }
    }

    console.log('IPFS options: ', options);
    const node = await IPFS.create(options);

    console.log('IPFS node is ready');
    window.ipfs = node;

    for (const video of document.getElementsByTagName('video'))
    {
        const src = video.dataset.ipfsSrc;
        if (src) {
            const path = src.split('/');

            // https://github.com/video-dev/hls.js/blob/master/docs/API.md
            const hls = new Hls({
                debug: true,
                enableWorker: false,
                manifestLoadingMaxRetry: 10,
                startLevel: -1,
                testBandwidth: false,
                capLevelToPlayerSize: true,
                loader: HlsjsIpfsLoader,
                ipfs: node,
                ipfsHash: path[0]
            });

            hls.config.ipfs = node;
            hls.config.ipfsHash = path[0];
            hls.loadSource(path[1]);
            hls.attachMedia(video);

            hls.on(Hls.Events.MANIFEST_PARSED, () => {
                try {
                    video.play();
                } catch (e) {
                    console.error(e);
                }
            });
        }
    }
});
