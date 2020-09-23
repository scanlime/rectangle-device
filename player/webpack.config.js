const webpack = require('webpack');
const BundleAnalyzerPlugin = require('webpack-bundle-analyzer').BundleAnalyzerPlugin;
const path = require('path');

const src_dir = path.resolve(process.env.CARGO_MANIFEST_DIR || __dirname, 'src');
const build_dir = path.resolve(process.env.OUT_DIR || __dirname);

const config = {
	entry: path.resolve(src_dir, 'index.js'),
	mode: 'production',
	performance: {
		hints: false
	},
	output: {
		path: path.resolve(build_dir, 'dist')
	},
	plugins: [
		// new BundleAnalyzerPlugin()
	],
	resolve: {
		'modules': [
			'node_modules',
			path.resolve(build_dir, 'node_modules')
		]
	}
};

module.exports = config;
