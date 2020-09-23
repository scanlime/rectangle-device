const webpack = require('webpack');
const BundleAnalyzerPlugin = require('webpack-bundle-analyzer').BundleAnalyzerPlugin;
const path = require('path');

const config = {
	entry: path.resolve(process.env.CARGO_MANIFEST_DIR || __dirname, 'src/index.js'),
	mode: 'production',
	performance: {
		hints: false
	},
	output: {
		path: path.resolve(process.env.OUT_DIR || __dirname, 'dist'),
	},
	plugins: [
		// new BundleAnalyzerPlugin()
	]
};

module.exports = config;
