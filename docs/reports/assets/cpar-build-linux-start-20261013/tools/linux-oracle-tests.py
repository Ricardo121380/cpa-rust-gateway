"""Evaluate the actual negative oracle against retained sanitized Linux evidence."""
import ast
import json
from pathlib import Path
import types
import unittest

PUBLIC = Path(__file__).resolve().parent.parent


def qualifies(number):
    row = json.loads((PUBLIC / ('system-' + str(number) + '.json')).read_text())['result']
    tree = ast.parse((PUBLIC / 'tools/linux-system-validation.py').read_text())
    assertion = next(node for node in tree.body if isinstance(node, ast.Assign)
                     and any(isinstance(target, ast.Name) and target.id == 'expected' for target in node.targets))
    return eval(compile(ast.Expression(assertion.value), 'actual-system-oracle', 'eval'), {
        'number': number, 'failure': row['failure_reason'], 'http_statuses': row['http_statuses'],
        'probe': types.SimpleNamespace(trace=row['startup_observations'])})


class LinuxOracle(unittest.TestCase):
    def test_retained_normal_run_qualifies(self):
        self.assertTrue(qualifies(1))

    def test_missing_exec_failure_status_never_becomes_pass(self):
        self.assertFalse(qualifies(2))

    def test_product_masked_management_rejection_is_the_expected_negative(self):
        self.assertTrue(qualifies(3), 'candidate management denial is 404, not 401')
        self.assertEqual(json.loads((PUBLIC / 'system-3.json').read_text())['result']['status'], 'FAIL',
                         'original real-run FAIL must remain unchanged')


if __name__ == '__main__':
    unittest.main(verbosity=2)
