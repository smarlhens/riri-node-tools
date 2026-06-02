export default {
  enginesNode: true,
  reject: [],
  target: name => {
    const targets = {
      '@types/node': 'minor',
      // Benchmark baselines: keep each alias on its own major line (v0→0.x, v1→1.x).
      'npm-check-engines': 'minor',
      'npm-pin-dependencies': 'minor',
    };

    const keys = Object.keys(targets);
    if (keys.some(key => new RegExp(key).test(name))) {
      return targets[keys.find(key => new RegExp(key).test(name))];
    }

    return 'latest';
  },
};
