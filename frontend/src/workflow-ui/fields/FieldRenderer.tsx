import { useContext } from 'react';
import type { Field, Aggregation } from './types';
import {
    BoolField,
    IntegerField,
    NumberField,
    SelectField,
    TextField,
    TextareaField,
} from './PrimitiveFields';
import { FilePathField } from './FilePathField';
import { ExpressionField } from './ExpressionField';
import { ColumnField, ColumnsField } from './ColumnField';
import { AggregationsField } from './AggregationsField';
import { KeyValueField } from './KeyValueField';
import { FilterBuilderField } from './FilterBuilderField';
import { ConnectionRefField } from './ConnectionRefField';
import { RoutineRefField } from './RoutineRefField';
import { FieldContext } from './FieldContext';

type Props = {
    field: Field;
    value: unknown;
    onChange: (v: unknown) => void;
};

// Free-value fields that can be bound to a context variable instead of
// typed in manually.
const BINDABLE = new Set([
    'text',
    'textarea',
    'number',
    'integer',
    'file-path',
    'save-path',
    'expression',
]);

export default function FieldRenderer({ field, value, onChange }: Props) {
    return (
        <div className="form-field">
            <label className="form-field-label">
                {field.label}
                {field.required ? <span className="form-field-required">*</span> : null}
            </label>
            {BINDABLE.has(field.kind) ? (
                <ContextBindable value={value} onChange={onChange}>
                    {(v, oc) => renderInput(field, v, oc)}
                </ContextBindable>
            ) : (
                renderInput(field, value, onChange)
            )}
            {field.description ? (
                <div className="form-field-desc">{field.description}</div>
            ) : null}
        </div>
    );
}

/**
 * Wraps a value field with a "Manual entry / context variable" source
 * picker when the project has an active context. Choosing a variable
 * sets the value to `${key}`, which is resolved at run time.
 */
function ContextBindable({
    value,
    onChange,
    children,
}: {
    value: unknown;
    onChange: (v: unknown) => void;
    children: (value: unknown, onChange: (v: unknown) => void) => React.ReactNode;
}) {
    const { activeContext } = useContext(FieldContext);
    const vars = activeContext?.variables ?? [];
    if (!activeContext || vars.length === 0) {
        return <>{children(value, onChange)}</>;
    }
    const strVal = typeof value === 'string' ? value : '';
    const match = strVal.match(/^\$\{\s*([^}]+?)\s*\}$/);
    const boundKey = match ? match[1] : '';
    const bound = boundKey.length > 0;
    return (
        <div className="ctx-bindable">
            <select
                className="schema-input ctx-source-select"
                value={bound ? boundKey : '__manual'}
                onChange={e => {
                    const v = e.target.value;
                    onChange(v === '__manual' ? '' : '${' + v + '}');
                }}
                title={`Field source (context: ${activeContext.name})`}
            >
                <option value="__manual">Manual entry</option>
                <optgroup label={activeContext.name}>
                    {vars.map(v => (
                        <option key={v.key} value={v.key}>
                            {v.key}
                            {v.secret ? ' = ••••' : ` = ${v.value}`}
                        </option>
                    ))}
                </optgroup>
            </select>
            {bound ? (
                <div className="ctx-bound" title="Resolved from the active context at run time">
                    <span className="ctx-bound-token">{'${' + boundKey + '}'}</span>
                    <span className="ctx-bound-hint">from {activeContext.name}</span>
                </div>
            ) : (
                children(value, onChange)
            )}
        </div>
    );
}

function renderInput(field: Field, value: unknown, onChange: (v: unknown) => void): React.ReactNode {
    switch (field.kind) {
        case 'text':
            return <TextField field={field} value={value as string | undefined} onChange={onChange} />;
        case 'textarea':
            return (
                <TextareaField field={field} value={value as string | undefined} onChange={onChange} />
            );
        case 'number':
            return <NumberField field={field} value={value as number | undefined} onChange={onChange} />;
        case 'integer':
            return (
                <IntegerField field={field} value={value as number | undefined} onChange={onChange} />
            );
        case 'bool':
            return <BoolField field={field} value={value as boolean | undefined} onChange={onChange} />;
        case 'select':
            return <SelectField field={field} value={value as string | undefined} onChange={onChange} />;
        case 'file-path':
            return (
                <FilePathField
                    field={field}
                    value={value as string | undefined}
                    onChange={onChange}
                    mode="open"
                />
            );
        case 'save-path':
            return (
                <FilePathField
                    field={field}
                    value={value as string | undefined}
                    onChange={onChange}
                    mode="save"
                />
            );
        case 'expression':
            return (
                <ExpressionField
                    field={field}
                    value={value as string | undefined}
                    onChange={onChange}
                />
            );
        case 'filter-predicate':
            return <FilterBuilderField value={value} onChange={onChange} />;
        case 'column':
            return (
                <ColumnField field={field} value={value as string | undefined} onChange={onChange} />
            );
        case 'columns':
            return (
                <ColumnsField field={field} value={value as string[] | undefined} onChange={onChange} />
            );
        case 'aggregations':
            return (
                <AggregationsField value={value as Aggregation[] | undefined} onChange={onChange} />
            );
        case 'key-value':
            return (
                <KeyValueField
                    value={value as { key: string; value: string }[] | undefined}
                    onChange={onChange}
                />
            );
        case 'connection-ref':
            return (
                <ConnectionRefField
                    field={field}
                    value={value as string | undefined}
                    onChange={onChange}
                />
            );
        case 'routine-ref':
            return (
                <RoutineRefField
                    field={field}
                    value={value as string | undefined}
                    onChange={onChange}
                />
            );
    }
}
