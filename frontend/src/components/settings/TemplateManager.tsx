// Added: Email template management UI for TMAIL-94
import React from 'react';
import { useState } from 'react';

import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { ArrowLeft, Plus, Trash2, Edit2, Eye } from 'lucide-react';
import {
  listTemplates,
  createTemplate,
  updateTemplate,
  deleteTemplate,
  renderTemplate,
} from '../../api/templates';
import type {
  EmailTemplate,
  CreateTemplateRequest,
  UpdateTemplateRequest,
  RenderTemplateRequest,
  RenderResult,
} from '../../api/templates';
import { useMailStore } from '../../stores/mailStore';
import { LoadingSkeleton } from '../shared/LoadingSkeleton';

/**
 * PURPOSE: Manage email templates — create, edit, delete, and preview with merge fields
 * EXTERNAL: Uses /api/templates endpoints via TanStack Query
 */
export function TemplateManager() {
  const queryClient = useQueryClient();
  const setViewMode = useMailStore((s) => s.setViewMode);

  // Added: UI state for form visibility, editing, and preview
  const [showForm, setShowForm] = useState(false);
  const [editing, setEditing] = useState<EmailTemplate | null>(null);
  const [previewTemplate, setPreviewTemplate] = useState<EmailTemplate | null>(null);
  const [previewResult, setPreviewResult] = useState<RenderResult | null>(null);
  const [previewFields, setPreviewFields] = useState<Record<string, string>>({});

  // Added: Form field state
  const [name, setName] = useState('');
  const [subject, setSubject] = useState('');
  const [bodyHtml, setBodyHtml] = useState('');
  const [bodyText, setBodyText] = useState('');
  const [mergeFieldsStr, setMergeFieldsStr] = useState('');
  const [category, setCategory] = useState('');
  const [isShared, setIsShared] = useState(false);

  const { data: templates = [], isLoading } = useQuery({
    queryKey: ['templates'],
    queryFn: listTemplates,
  });

  const createMutation = useMutation({
    mutationFn: createTemplate,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['templates'] });
      resetForm();
    },
  });

  const updateMutation = useMutation({
    mutationFn: ({ id, data }: { id: string; data: UpdateTemplateRequest }) =>
      updateTemplate(id, data),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['templates'] });
      resetForm();
    },
  });

  const deleteMutation = useMutation({
    mutationFn: deleteTemplate,
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['templates'] }),
  });

  const renderMutation = useMutation({
    mutationFn: ({ id, data }: { id: string; data: RenderTemplateRequest }) =>
      renderTemplate(id, data),
    onSuccess: (result: RenderResult) => {
      setPreviewResult(result);
    },
  });

  // Added: Reset form fields and close form
  const resetForm = () => {
    setShowForm(false);
    setEditing(null);
    setName('');
    setSubject('');
    setBodyHtml('');
    setBodyText('');
    setMergeFieldsStr('');
    setCategory('');
    setIsShared(false);
  };

  // Added: Populate form fields from an existing template for editing
  const startEditing = (template: EmailTemplate) => {
    setEditing(template);
    setShowForm(true);
    setName(template.name);
    setSubject(template.subject);
    setBodyHtml(template.body_html);
    setBodyText(template.body_text);
    setMergeFieldsStr(template.merge_fields.join(', '));
    setCategory(template.category || '');
    setIsShared(template.is_shared);
  };

  // Added: Open preview panel and initialize merge field inputs
  const startPreview = (template: EmailTemplate) => {
    setPreviewTemplate(template);
    setPreviewResult(null);
    const fields: Record<string, string> = {};
    template.merge_fields.forEach((fieldName) => {
      fields[fieldName] = '';
    });
    setPreviewFields(fields);
  };

  const handleSubmit = (e: React.FormEvent<HTMLFormElement>) => {
    e.preventDefault();
    // NOTE: Parse comma-separated merge fields into array, trimming whitespace
    const mergeFields = mergeFieldsStr
      .split(',')
      .map((f) => f.trim())
      .filter(Boolean);

    const data: CreateTemplateRequest = {
      name,
      subject,
      body_html: bodyHtml,
      body_text: bodyText,
      merge_fields: mergeFields.length > 0 ? mergeFields : undefined,
      category: category || undefined,
      is_shared: isShared,
    };

    if (editing) {
      updateMutation.mutate({ id: editing.id, data });
    } else {
      createMutation.mutate(data);
    }
  };

  // Added: Submit render preview with current merge field values
  const handleRenderPreview = (e: React.FormEvent<HTMLFormElement>) => {
    e.preventDefault();
    if (!previewTemplate) return;
    renderMutation.mutate({
      id: previewTemplate.id,
      data: { fields: previewFields },
    });
  };

  if (isLoading) return <LoadingSkeleton rows={4} />;

  return (
    <div className="template-manager" style={{ padding: '16px', maxWidth: '800px' }}>
      <div className="message-view__toolbar">
        <button className="btn btn--icon" onClick={() => setViewMode('list')} title="Back">
          <ArrowLeft size={20} />
        </button>
        <h2 style={{ flex: 1, fontSize: '18px' }}>Email Templates</h2>
        {!showForm && (
          <button
            className="btn btn--primary"
            onClick={() => {
              resetForm();
              setShowForm(true);
            }}
          >
            <Plus size={16} /> New Template
          </button>
        )}
      </div>

      {/* Added: Create/edit form */}
      {showForm && (
        <form
          onSubmit={handleSubmit}
          style={{
            marginTop: '16px',
            padding: '16px',
            border: '1px solid var(--color-border)',
            borderRadius: '8px',
          }}
        >
          <h3 style={{ marginBottom: '12px' }}>
            {editing ? 'Edit Template' : 'New Template'}
          </h3>
          <div className="composer__field">
            <label>Name</label>
            <input
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder="e.g., Welcome Email"
              required
              data-testid="template-name"
            />
          </div>
          <div className="composer__field">
            <label>Subject</label>
            <input
              value={subject}
              onChange={(e) => setSubject(e.target.value)}
              placeholder="Email subject line"
              required
              data-testid="template-subject"
            />
          </div>
          <div className="composer__field">
            <label>HTML Body</label>
            <textarea
              value={bodyHtml}
              onChange={(e) => setBodyHtml(e.target.value)}
              placeholder="<h1>Hello {{name}}</h1>"
              rows={6}
              required
              data-testid="template-body-html"
            />
          </div>
          <div className="composer__field">
            <label>Text Body</label>
            <textarea
              value={bodyText}
              onChange={(e) => setBodyText(e.target.value)}
              placeholder="Hello {{name}}"
              rows={3}
              data-testid="template-body-text"
            />
          </div>
          <div className="composer__field">
            <label>Merge Fields (comma-separated)</label>
            <input
              value={mergeFieldsStr}
              onChange={(e) => setMergeFieldsStr(e.target.value)}
              placeholder="name, email, company"
              data-testid="template-merge-fields"
            />
          </div>
          <div className="composer__field">
            <label>Category</label>
            <input
              value={category}
              onChange={(e) => setCategory(e.target.value)}
              placeholder="e.g., Marketing"
              data-testid="template-category"
            />
          </div>
          <label
            style={{
              display: 'flex',
              alignItems: 'center',
              gap: '8px',
              marginBottom: '16px',
            }}
          >
            <input
              type="checkbox"
              checked={isShared}
              onChange={(e) => setIsShared(e.target.checked)}
              data-testid="template-is-shared"
            />
            Share with team
          </label>
          <div className="composer__actions" style={{ display: 'flex', gap: '8px' }}>
            <button
              type="submit"
              className="btn btn--primary"
              disabled={createMutation.isPending || updateMutation.isPending}
            >
              {editing ? 'Update Template' : 'Create Template'}
            </button>
            <button type="button" className="btn" onClick={resetForm}>
              Cancel
            </button>
          </div>
        </form>
      )}

      {/* Added: Render preview panel */}
      {previewTemplate && (
        <div
          style={{
            marginTop: '16px',
            padding: '16px',
            border: '1px solid var(--color-border)',
            borderRadius: '8px',
            background: 'var(--color-bg-secondary)',
          }}
        >
          <h3 style={{ marginBottom: '12px' }}>
            Preview: {previewTemplate.name}
          </h3>
          <form onSubmit={handleRenderPreview}>
            {previewTemplate.merge_fields.map((field) => (
              <div className="composer__field" key={field}>
                <label>{field}</label>
                <input
                  value={previewFields[field] || ''}
                  onChange={(e) =>
                    setPreviewFields({ ...previewFields, [field]: e.target.value })
                  }
                  placeholder={`Value for ${field}`}
                  data-testid={`preview-field-${field}`}
                />
              </div>
            ))}
            <div style={{ display: 'flex', gap: '8px', marginTop: '8px' }}>
              <button
                type="submit"
                className="btn btn--primary"
                disabled={renderMutation.isPending}
              >
                {renderMutation.isPending ? 'Rendering...' : 'Render Preview'}
              </button>
              <button
                type="button"
                className="btn"
                onClick={() => {
                  setPreviewTemplate(null);
                  setPreviewResult(null);
                }}
              >
                Close Preview
              </button>
            </div>
          </form>
          {/* Added: Rendered preview output — content is server-sanitized */}
          {previewResult && (
            <div style={{ marginTop: '16px' }}>
              <h4>Subject: {previewResult.subject}</h4>
              <div
                style={{
                  marginTop: '8px',
                  padding: '12px',
                  border: '1px solid var(--color-border)',
                  borderRadius: '4px',
                  background: 'white',
                }}
                data-testid="preview-output"
              >
                {previewResult.body_text}
              </div>
            </div>
          )}
        </div>
      )}

      {/* Added: Template list */}
      <div style={{ marginTop: '16px' }}>
        {templates.length === 0 && !showForm && (
          <p
            style={{
              color: 'var(--color-text-secondary)',
              textAlign: 'center',
              padding: '24px',
            }}
          >
            No templates yet. Create one to speed up your email workflow.
          </p>
        )}
        {templates.map((template) => (
          <div
            key={template.id}
            style={{
              display: 'flex',
              alignItems: 'center',
              gap: '12px',
              padding: '12px',
              borderBottom: '1px solid var(--color-border)',
            }}
          >
            <div style={{ flex: 1 }}>
              <strong>{template.name}</strong>
              {template.is_shared && (
                <span
                  style={{
                    marginLeft: '8px',
                    fontSize: '11px',
                    background: 'var(--color-success, #28a745)',
                    color: 'white',
                    padding: '1px 6px',
                    borderRadius: '10px',
                  }}
                >
                  Shared
                </span>
              )}
              <div
                style={{
                  fontSize: '12px',
                  color: 'var(--color-text-secondary)',
                  marginTop: '4px',
                }}
              >
                {template.subject}
                {template.category && ` · ${template.category}`}
                {template.merge_fields.length > 0 &&
                  ` · ${template.merge_fields.length} merge field${template.merge_fields.length !== 1 ? 's' : ''}`}
              </div>
            </div>
            <button
              className="btn btn--icon"
              onClick={() => startPreview(template)}
              title="Preview"
              data-testid={`preview-${template.id}`}
            >
              <Eye size={16} />
            </button>
            <button
              className="btn btn--icon"
              onClick={() => startEditing(template)}
              title="Edit"
              data-testid={`edit-${template.id}`}
            >
              <Edit2 size={16} />
            </button>
            <button
              className="btn btn--icon btn--danger"
              onClick={() => deleteMutation.mutate(template.id)}
              title="Delete"
              data-testid={`delete-${template.id}`}
            >
              <Trash2 size={16} />
            </button>
          </div>
        ))}
      </div>
    </div>
  );
}
