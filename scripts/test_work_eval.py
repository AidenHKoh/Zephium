"""Harmless fabricated fixtures; no network, signing, native app, or model calls."""
import copy
import importlib.machinery
import importlib.util
import json
import os
from pathlib import Path
import tempfile
import unittest

loader = importlib.machinery.SourceFileLoader('work_eval', str(Path(__file__).with_name('work-eval')))
spec = importlib.util.spec_from_loader(loader.name, loader)
eval = importlib.util.module_from_spec(spec)
loader.exec_module(eval)


def fixture():
    link = {'extraction_id': 'fixture', 'source_id': 1}
    task = {'id': 'fixture', 'urls': ['https://example.test/facts'], 'grading': 'deterministic', 'predicates': [{'id': 'p1'}]}
    model = {'model': {'provider': 'open_ai', 'model': eval.MODEL}}
    source = {'id': 'source', 'published': False, 'data': {'kind': 'findings'}, 'evidence': [link]}
    answer = {'id': 'answer', 'published': True, 'data': {'kind': 'code', 'language': 'json', 'text': '{"title":"Fixture Title","count":3}'}, 'evidence': [link]}
    step = {'id': 'read', 'kind': 'read', 'status': 'succeeded', 'url': task['urls'][0], 'artifacts': ['source'], 'measurements': {'planner_calls': 2, 'outcome': {'kind': 'completed'}}}
    evidence = {'schema_version': 1, 'case_id': 'fixture', 'public_artifacts': 'exported', 'models': {role: copy.deepcopy(model) for role in ('lead', 'page', 'light')},
                'wall_ms': 10, 'model_calls': [{'state': 'settled'}, {'state': 'unsettled'}], 'executions': [{'status': 'needs_review', 'steps': [step], 'artifacts': [source, answer],
                'attempts': [{'status': 'succeeded', 'usage': {'model_tokens': 10, 'cost_micro_usd': 20, 'accounting': 'conservative_reservation'}}]}]}
    return task, evidence, {'state': 'ready', 'expected': {'title': 'Fixture Title', 'count': 3}}, {'excluded_domains': ['tuned.test']}


class GradingTests(unittest.TestCase):
    def test_task_pass_requires_values_and_native_source_links(self):
        task, evidence, reference, manifest = fixture()
        self.assertEqual(eval.grade(task, evidence, reference, manifest)['task_outcome'], 'passed')
        evidence['executions'][0]['status'] = 'failed'
        self.assertEqual(eval.grade(task, evidence, reference, manifest)['task_outcome'], 'passed', 'operational and semantic outcomes are separate')
        evidence['executions'][0]['artifacts'][1]['data']['text'] = '{"title":"Wrong","count":3}'
        self.assertEqual(eval.grade(task, evidence, reference, manifest)['task_outcome'], 'failed')

    def test_success_objects_without_verified_values_do_not_pass(self):
        task, evidence, reference, manifest = fixture()
        evidence['executions'][0]['artifacts'][1]['data']['text'] = 'I finished it successfully.'
        self.assertEqual(eval.grade(task, evidence, reference, manifest)['task_outcome'], 'failed')
        task['grading'] = 'source_audit'
        self.assertEqual(eval.grade(task, evidence, reference, manifest)['task_outcome'], 'unknown')

    def test_missing_or_unrelated_source_evidence_fails(self):
        for mutation in ('url', 'read_status', 'link'):
            task, evidence, reference, manifest = fixture()
            if mutation == 'url': evidence['executions'][0]['steps'][0]['url'] = 'https://different.test/facts'
            if mutation == 'read_status': evidence['executions'][0]['steps'][0]['status'] = 'failed'
            if mutation == 'link': evidence['executions'][0]['artifacts'][1]['evidence'] = []
            self.assertEqual(eval.grade(task, evidence, reference, manifest)['task_outcome'], 'failed')

    def test_unrelated_published_object_cannot_supply_answer_provenance(self):
        task, evidence, reference, manifest = fixture()
        artifacts = evidence['executions'][0]['artifacts']
        unrelated = copy.deepcopy(artifacts[0])
        unrelated.update(id='unrelated', published=True)
        artifacts.append(unrelated)
        artifacts[1]['evidence'] = []
        self.assertEqual(eval.grade(task, evidence, reference, manifest)['task_outcome'], 'failed')

    def test_revision_inherits_publication_but_not_old_correctness_or_citations(self):
        task, evidence, reference, manifest = fixture()
        artifacts = evidence['executions'][0]['artifacts']
        for artifact in artifacts:
            artifact['revises'] = None
        revised = copy.deepcopy(artifacts[1])
        revised.update(id='revised', revises='answer', published=False)
        revised['data']['text'] = '{"title":"Wrong","count":3}'
        artifacts.append(revised)
        self.assertEqual(eval.grade(task, evidence, reference, manifest)['task_outcome'], 'failed')
        revised['data']['text'] = artifacts[1]['data']['text']
        self.assertEqual(eval.grade(task, evidence, reference, manifest)['task_outcome'], 'passed')
        revised['evidence'] = []
        self.assertEqual(eval.grade(task, evidence, reference, manifest)['task_outcome'], 'failed')

    def test_ambiguous_legacy_answers_and_broken_revision_history_stay_unknown(self):
        task, evidence, reference, manifest = fixture()
        artifacts = evidence['executions'][0]['artifacts']
        second = copy.deepcopy(artifacts[1])
        second['id'] = 'second'
        artifacts.append(second)
        self.assertEqual(eval.grade(task, evidence, reference, manifest)['reason'], 'legacy_artifact_revisions_unknown')
        for artifact in artifacts:
            artifact['revises'] = None
        second['revises'] = 'missing'
        self.assertEqual(eval.grade(task, evidence, reference, manifest)['reason'], 'invalid_artifact_revision_history')
        second['revises'] = 'answer'
        artifacts[1]['revises'] = 'second'
        self.assertEqual(eval.grade(task, evidence, reference, manifest)['reason'], 'invalid_artifact_revision_history')

    def test_private_wrong_model_and_tuned_contamination_stay_unknown(self):
        for mutation in ('private', 'model', 'tuned', 'case'):
            task, evidence, reference, manifest = fixture()
            if mutation == 'private': evidence['public_artifacts'] = 'withheld_private_provenance'
            if mutation == 'model': evidence['models']['page']['model']['model'] = 'other'
            if mutation == 'tuned': evidence['executions'][0]['steps'][0]['url'] = 'https://www.tuned.test/facts'
            if mutation == 'case': evidence['case_id'] = 'other'
            self.assertEqual(eval.grade(task, evidence, reference, manifest)['task_outcome'], 'unknown')

    def test_exact_keys_and_scalar_types(self):
        task, evidence, reference, manifest = fixture()
        for text in ('{"title":"Fixture Title","count":"3"}', '{"title":"Fixture Title","count":3,"extra":1}', '{"title":"Fixture Title"}'):
            evidence['executions'][0]['artifacts'][1]['data']['text'] = text
            self.assertEqual(eval.grade(task, evidence, reference, manifest)['task_outcome'], 'failed')

    def test_internal_refusals_remain_visible_after_correct_task_result(self):
        _, evidence, _, _ = fixture()
        log = 'lead-run: event=ObjectRefused { reason: Some(ReplyMarkup) }\n' * 2
        metrics = eval.metrics(evidence, log)
        self.assertEqual(metrics['object_refusal_reasons'], {'ReplyMarkup': 2})
        self.assertEqual(metrics['reported_cost_micro_usd'], 20)
        self.assertEqual(metrics['accounting_bases'], ['conservative_reservation'])
        self.assertEqual(metrics['selected_model_call_attempts'], 2)
        self.assertEqual(metrics['unsettled_model_calls'], 1)
        self.assertEqual(metrics['page_model_calls'], 2)

    def test_missing_calls_are_unknown_and_emulation_is_not_added_twice(self):
        self.assertIsNone(eval.metrics({}, '')['selected_model_call_attempts'])
        _, evidence, _, _ = fixture()
        evidence['executions'][0]['steps'][0]['measurements'].update(decision_calls=2,emulation_calls=1)
        row=eval.metrics(evidence,'')
        self.assertEqual(row['selected_model_call_attempts'],2)
        self.assertEqual(row['primary_decision_calls'],2)
        self.assertEqual(row['emulation_calls_overlapping_selected_client'],1)

    def test_missing_usage_is_unknown_not_zero(self):
        _, evidence, _, _ = fixture()
        evidence['executions'][0]['attempts'][0]['usage'] = None
        self.assertIsNone(eval.metrics(evidence, '')['reported_cost_micro_usd'])


class ReferenceAndFreezeTests(unittest.TestCase):
    def test_reference_extraction_is_independent_json_and_html(self):
        source = {'format': 'json', 'fields': {'count': {'path': ['items'], 'transform': 'length'}}}
        self.assertEqual(eval.reference_fields(source, b'{"items":[1,2]}'), {'count': 2})
        source = {'format': 'html_meta', 'fields': {'title': {'path': ['citation_title', 0]}, 'authors': {'path': ['citation_author'], 'transform': 'length'}}}
        body = b'<meta name="citation_title" content="Fixture"><meta name="citation_author" content="A"><meta name="citation_author" content="B">'
        self.assertEqual(eval.reference_fields(source, body), {'title': 'Fixture', 'authors': 2})

    def test_private_files_are_exclusive_and_0600(self):
        with tempfile.TemporaryDirectory() as folder:
            path = Path(folder, 'evidence.json')
            eval.write_private(path, {'fixture': True})
            self.assertEqual(path.stat().st_mode & 0o777, 0o600)
            with self.assertRaises(FileExistsError): eval.write_private(path, {'overwritten': True})
            self.assertEqual(eval.read_json(path), {'fixture': True})

    def test_manifest_excludes_all_23_tuned_cases_and_recipe_travel_hosts(self):
        manifest = eval.load_manifest(Path(__file__).parent.parent / 'eval/work-heldout-v1.json')
        self.assertEqual(len(manifest['tasks']), 100)
        self.assertEqual(len(manifest['tuned_cases']), 23)
        self.assertIn('unsplash', {case['id'] for case in manifest['tuned_cases']})
        self.assertEqual(sum(task['eligibility'] == 'eligible_public' for task in manifest['tasks']), 70)
        for url in ('https://www.ebay.com/', 'https://unsplash.com/s/photos/other', 'https://www.kayak.com/', 'https://flights.google.com/'):
            self.assertTrue(eval.excluded(url, manifest))
        self.assertFalse(eval.excluded('https://not-ebay.com/', manifest))

    def test_summary_does_not_count_pending_or_manual_as_passes(self):
        _, evidence, _, _ = fixture()
        rows = [{'id': 'fixture', 'eligibility': 'eligible_public', 'exit': 0, 'process_completed': True, 'process_ms': 10, 'task_outcome': 'unknown', 'metrics': eval.metrics(evidence, '')},
                {'id': 'account', 'eligibility': 'pending_user_session', 'task_outcome': 'pending'}]
        summary = eval.summarize(rows)
        self.assertEqual(summary['verified_passes_of_attempted'], '0/1')
        self.assertEqual(summary['process_completions'], 1)


class FormOracleTests(unittest.TestCase):
    def receipt(self, checks, **changes):
        row = {"version": 1, "case": "form-selenium-controls", "reason": "observed", "checks": checks, "content_exported": False}
        row.update(changes)
        return "form-oracle: " + json.dumps(row) + "\n"

    def grade(self, log):
        return eval.grade_form({"id": "form-selenium-controls"}, {"case_id": "form-selenium-controls", "public_artifacts": "exported"}, log)

    def effects(self, **changes):
        row = {"version":1,"native_actions_started":2,"native_actions_settled":2,
               "native_commits_applied":0,"incomplete":False,
               "navigation":{"callbacks":2,"accepted_main_forms":0,"accepted_subframe_forms":0,
                             "post_initial_main":0,"consent_navigation":0,"initial_finished":True,"uncertain":False},
               "no_task_submit_or_navigation":True,"fill_then_clear":False,"fill_then_increment_once":False}
        row.update(changes)
        return row

    def settled_receipt(self, checks, effects=None, event=True, task="form-selenium-controls"):
        effects = effects or self.effects()
        return self.receipt(checks, case=task, effects=effects, no_task_submit_event=event) + "form-effects: " + json.dumps({"version":1,"case":task,"effects":effects,"content_exported":False}) + "\n"

    def test_complete_native_effects_and_submit_event_qualify_exact_p2(self):
        self.assertEqual(self.grade(self.settled_receipt([True,True,True]))['task_outcome'], 'passed')
        self.assertEqual(self.grade(self.settled_receipt([True,True,True],event=False))['predicates'][1]['state'], 'failed')
        self.assertEqual(self.grade(self.settled_receipt([True,True,True],event=None))['predicates'][1]['state'], 'unknown')
        commit = self.effects(native_commits_applied=1,no_task_submit_or_navigation=False)
        self.assertEqual(self.grade(self.settled_receipt([True,True,True],effects=commit))['task_outcome'], 'failed')
        for changes in ({"incomplete":True}, {"native_actions_settled":1}, {"native_actions_started":257}):
            self.assertEqual(self.grade(self.settled_receipt([True,True,True],effects=self.effects(**changes)))['predicates'][1]['state'], 'unknown')
        missing = self.receipt([True,True,True],effects=self.effects(),no_task_submit_event=True)
        self.assertEqual(self.grade(missing)['predicates'][1]['state'], 'unknown')
        self.assertEqual(self.grade(self.settled_receipt([True,True,True])+missing)['task_outcome'], 'unknown')

    def test_unique_action_resource_requires_all_page_receipts_and_exact_stamps(self):
        readonly = self.effects(native_actions_started=0,native_actions_settled=0)
        def page(checks, effects, stamp, event=True):
            oracle=self.receipt(checks, effects=effects, no_task_submit_event=event,
                                document=f"WorkDocumentStamp {{ native_id: NavigationId({stamp}), epoch: 1 }}", profile="synthetic-profile")
            return oracle+"form-effects: "+json.dumps({"version":1,"case":"form-selenium-controls","effects":effects,"content_exported":False})+"\n"
        initial=page([False,False,True],readonly,1)
        action=page([True,True,True],self.effects(),2)
        result=self.grade(initial+action)
        self.assertEqual(result['task_outcome'],'passed')
        self.assertEqual(result['form_resource_selection'],'unique_action_page_with_every_opened_page_accounted')
        self.assertEqual(self.grade(initial+page([True,True,True],self.effects(),2,event=None))['task_outcome'],'unknown')
        self.assertEqual(self.grade(page([False,False,True],readonly,1,event=False)+action)['task_outcome'],'failed')
        for log in (action+page([True,True,True],self.effects(),3),
                    initial+page([True,True,True],self.effects(),1),
                    initial+action.split('form-effects: ')[0],
                    initial+page([True,True,True],self.effects(incomplete=True,no_task_submit_or_navigation=None),2),
                    initial+page([True,True,True],readonly,2)):
            self.assertEqual(self.grade(log)['task_outcome'],'unknown')

    def test_sequence_needs_target_bound_settled_history(self):
        for task, checks, field in (("form-expand-inputs",[True,True,None],"fill_then_clear"),
                                    ("form-heroku-numeric",[True,None],"fill_then_increment_once")):
            evidence={"case_id":task,"public_artifacts":"exported"}
            log=self.settled_receipt(checks, self.effects(**{field:True}), task=task)
            self.assertEqual(eval.grade_form({"id":task},evidence,log)['task_outcome'],'passed')
            log=self.settled_receipt(checks, self.effects(**{field:False}), task=task)
            self.assertEqual(eval.grade_form({"id":task},evidence,log)['task_outcome'],'failed')
            log=self.settled_receipt(checks, self.effects(incomplete=True,no_task_submit_or_navigation=None,**{field:True}), task=task)
            self.assertEqual(eval.grade_form({"id":task},evidence,log)['task_outcome'],'unknown')

    def test_correct_final_values_are_not_network_non_submission_proof(self):
        row = self.grade(self.receipt([True, True, True]))
        self.assertEqual(row['predicates'][0]['state'], 'passed')
        self.assertEqual(row['predicates'][1]['state'], 'unknown')
        self.assertEqual(row['task_outcome'], 'unknown')

    def test_fresh_wrong_value_fails_but_stale_or_multiple_pages_stay_unknown(self):
        self.assertEqual(self.grade(self.receipt([True, False, True]))['task_outcome'], 'failed')
        for log in (self.receipt([True, False, True], reason='stale_document'),
                    self.receipt([True, True, True]) + self.receipt([True, False, True]),
                    self.receipt(None, reason='timed_out'), ''):
            self.assertEqual(self.grade(log)['task_outcome'], 'unknown')

    def test_unknown_sequence_and_malformed_payload_never_pass(self):
        self.assertEqual(self.grade(self.receipt([True, None, True]))['predicates'][0]['state'], 'unknown')
        for checks in ([True], [True, 'true', True], [True, 1, True]):
            self.assertEqual(self.grade(self.receipt(checks))['task_outcome'], 'unknown')
        self.assertEqual(self.grade(self.receipt([True, True, True], content_exported=True))['task_outcome'], 'unknown')


if __name__ == '__main__':
    unittest.main()
