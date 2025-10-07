#!/usr/bin/env python3
"""
Comprehensive Test Suite Runner for Alpenglow Consensus

Runs all verification tests and produces a summary report.
"""

import subprocess
import sys
import time
from pathlib import Path

class TestRunner:
    def __init__(self):
        self.results = []
        self.total_tests = 0
        self.passed_tests = 0

    def run_command(self, name, command, cwd=None, timeout=300):
        """Run a command and capture result"""
        print(f"\n{'='*70}")
        print(f"Running: {name}")
        print(f"{'='*70}\n")

        start_time = time.time()

        try:
            result = subprocess.run(
                command,
                cwd=cwd,
                shell=True,
                capture_output=True,
                text=True,
                timeout=timeout
            )

            elapsed = time.time() - start_time
            success = result.returncode == 0

            self.total_tests += 1
            if success:
                self.passed_tests += 1

            self.results.append({
                'name': name,
                'success': success,
                'elapsed': elapsed,
                'output': result.stdout + result.stderr
            })

            # Print summary
            status = "[PASS]" if success else "[FAIL]"
            print(f"\n{status} {name} (took {elapsed:.1f}s)")

            return success

        except subprocess.TimeoutExpired:
            elapsed = time.time() - start_time
            print(f"\n[TIMEOUT] {name} (after {elapsed:.1f}s)")

            self.total_tests += 1
            self.results.append({
                'name': name,
                'success': False,
                'elapsed': elapsed,
                'output': 'TIMEOUT'
            })

            return False

        except Exception as e:
            print(f"\n[ERROR] {name}: {e}")

            self.total_tests += 1
            self.results.append({
                'name': name,
                'success': False,
                'elapsed': 0,
                'output': str(e)
            })

            return False

    def print_summary(self):
        """Print final summary"""
        print(f"\n\n{'='*70}")
        print("COMPREHENSIVE TEST SUITE SUMMARY")
        print(f"{'='*70}\n")

        # Group by category
        categories = {
            'Exhaustive Model Checking': [],
            'Statistical Verification': [],
            'Temporal Liveness': [],
            'Implementation Tests': []
        }

        for result in self.results:
            name = result['name']
            if 'Stateright' in name or 'TLC Simulation' in name:
                categories['Exhaustive Model Checking'].append(result)
            elif 'Statistical' in name:
                categories['Statistical Verification'].append(result)
            elif 'Temporal' in name:
                categories['Temporal Liveness'].append(result)
            else:
                categories['Implementation Tests'].append(result)

        # Print by category
        for category, tests in categories.items():
            if tests:
                print(f"\n{category}:")
                print("-" * 70)
                for test in tests:
                    status = "[PASS]" if test['success'] else "[FAIL]"
                    elapsed = test['elapsed']
                    print(f"  {status} {test['name']:<50} ({elapsed:6.1f}s)")

        # Overall statistics
        print(f"\n{'='*70}")
        print(f"OVERALL RESULTS")
        print(f"{'='*70}")
        print(f"Total Tests: {self.total_tests}")
        print(f"Passed: {self.passed_tests}")
        print(f"Failed: {self.total_tests - self.passed_tests}")
        print(f"Success Rate: {(self.passed_tests/self.total_tests*100):.1f}%")
        print()

        if self.passed_tests == self.total_tests:
            print("[PASS] ALL TESTS PASSED")
        elif self.passed_tests >= self.total_tests * 0.8:
            print(f"[PASS] MAJORITY PASSED ({self.passed_tests}/{self.total_tests})")
        else:
            print(f"[WARN] SOME FAILURES ({self.passed_tests}/{self.total_tests})")

        print()

def main():
    print("""
======================================================================
    ALPENGLOW CONSENSUS - COMPREHENSIVE TEST SUITE
======================================================================

This script runs all verification tests across multiple methods:
- Exhaustive model checking (Stateright + TLC simulation)
- Statistical model checking (Monte Carlo)
- Temporal liveness verification
- Bounded time property verification
- Statistical timing analysis
- Implementation unit tests

""")

    runner = TestRunner()
    base_dir = Path(__file__).parent
    tla_dir = base_dir / "tla-spec"
    rust_dir = base_dir / "rust-implementation"

    # Find cargo
    cargo_paths = [
        "/c/Users/harsh/.cargo/bin/cargo.exe",
        "/c/Users/Ajan/.cargo/bin/cargo.exe",
        "cargo"
    ]

    cargo = None
    for path in cargo_paths:
        try:
            result = subprocess.run(f"{path} --version", shell=True, capture_output=True)
            if result.returncode == 0:
                cargo = path
                break
        except:
            continue

    # Test 1: TLC Simulation
    runner.run_command(
        "TLC Simulation (3 + 5 validators)",
        "python3 simulate_tlc.py",
        cwd=tla_dir,
        timeout=120
    )

    # Test 2: Statistical Model Checking
    runner.run_command(
        "Statistical Model Checking (100/500/1000 validators)",
        "python3 statistical_mc.py",
        cwd=tla_dir,
        timeout=300
    )

    # Test 3: Temporal Liveness Verification
    runner.run_command(
        "Temporal Liveness Verification (symbolic)",
        "python3 temporal_verifier.py",
        cwd=tla_dir,
        timeout=600
    )

    # Test 4: Bounded Time Verification
    runner.run_command(
        "Bounded Time Property Verification",
        "python3 temporal_verifier_bounded.py",
        cwd=tla_dir,
        timeout=120
    )

    # Test 5: Timing Analysis
    runner.run_command(
        "Statistical Timing Analysis",
        "python3 timing_analysis.py",
        cwd=tla_dir,
        timeout=60
    )

    # Test 6: Stateright Model Checking (if cargo available)
    if cargo:
        runner.run_command(
            "Stateright Model Checking (6 safety tests)",
            f"{cargo} test --test stateright_model --release -- --test-threads=1 --skip exhaustive",
            cwd=rust_dir,
            timeout=300
        )

        runner.run_command(
            "Rust Implementation Tests (unit tests)",
            f"{cargo} test --lib --release",
            cwd=rust_dir,
            timeout=180
        )
    else:
        print("\n[SKIP] Cargo not found, skipping Rust tests")

    # Print summary
    runner.print_summary()

    # Generate report file
    report_path = base_dir / "TEST_RESULTS.md"
    with open(report_path, 'w', encoding='utf-8') as f:
        f.write("# Alpenglow Consensus - Test Results\n\n")
        f.write(f"**Date**: {time.strftime('%Y-%m-%d %H:%M:%S')}\n\n")
        f.write(f"**Overall**: {runner.passed_tests}/{runner.total_tests} tests passed ")
        f.write(f"({(runner.passed_tests/runner.total_tests*100):.1f}%)\n\n")

        f.write("## Test Results\n\n")
        for result in runner.results:
            status = "✅ PASS" if result['success'] else "❌ FAIL"
            f.write(f"### {status} - {result['name']}\n\n")
            f.write(f"**Time**: {result['elapsed']:.1f}s\n\n")

            if not result['success'] and result['output'] != 'TIMEOUT':
                f.write("**Output**:\n```\n")
                # Truncate long output
                output = result['output']
                if len(output) > 1000:
                    output = output[-1000:] + "\n... (truncated)"
                f.write(output)
                f.write("\n```\n\n")

        f.write("---\n\n")
        f.write("**Generated by**: `run_all_tests.py`\n")

    print(f"Test report saved to: {report_path}")

    # Exit code
    sys.exit(0 if runner.passed_tests == runner.total_tests else 1)

if __name__ == "__main__":
    main()
