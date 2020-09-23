const webpack = require('webpack');
const BundleAnalyzerPlugin = require('webpack-bundle-analyzer').BundleAnalyzerPlugin;
const path = require('path');

const config = {
	entry: './src/index.js',
	mode: 'production',
	performance: {
		hints: false
	},
	output: {
		path: process.env.OUT_DIR || path.resolve(__dirname, 'dist'),
	},
	plugins: [
		// new BundleAnalyzerPlugin()
	]
};

module.exports = config;
